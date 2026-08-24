//! Deterministic, non-authoritative Source/Product resolution identity projections.
//!
//! These projections centralize canonical hash preimages shared by the SBF
//! adapter and offchain transaction builders. They authenticate nothing: every
//! field is caller supplied, and equality with a projected digest proves only
//! that both callers hashed the same bytes. A live adapter must first derive
//! every field from hostile-authenticated accounts and exact semantic joins.

use clutch_product_series::ContentId;
use sha2::{Digest, Sha256};

use crate::RuntimeKey;

const SOURCE_PRODUCT_ROUTE_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/source-product-route-authentication/v4";
const SOURCE_RESOLUTION_INPUT_DOMAIN_V4: &[u8] = b"dragons-clutch/source-resolution-input/v4";

/// Untrusted complete preimage of the current Source/Product route identity.
///
/// This is a pure interoperability projection, not an authenticated route and
/// not authority to publish or resolve a Source occurrence. The SBF adapter
/// must populate it only after authenticating the Source release, receiver,
/// Registry capability, compiled Product bundle, Genesis, and Realm collateral.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceProductRouteIdProjectionV4 {
    /// Authenticated Source runtime route identity claimed by the caller.
    pub source_route_id: ContentId,
    /// Authenticated receiver-route identity claimed by the caller.
    pub receiver_route_id: ContentId,
    /// Immutable Source release manifest identity claimed by the caller.
    pub source_release_manifest_id: ContentId,
    /// Source release authentication receipt claimed by the caller.
    pub source_release_authentication_id: ContentId,
    /// Source-plane program contract identity claimed by the caller.
    pub source_plane_contract_id: ContentId,
    /// Source specification identity claimed by the caller.
    pub source_spec_id: ContentId,
    /// Current Registry release identity claimed by the caller.
    pub registry_release_id: ContentId,
    /// Current capability-profile identity claimed by the caller.
    pub capability_profile_id: ContentId,
    /// Current compiled Product/Series bundle identity claimed by the caller.
    pub compiler_bundle_id: ContentId,
    /// Current Market Genesis profile identity claimed by the caller.
    pub market_genesis_profile_id: ContentId,
    /// Immutable Realm identity claimed by the caller.
    pub realm_id: ContentId,
    /// Immutable profile identity claimed by the caller.
    pub profile_id: ContentId,
    /// Realm-selected collateral mint identity claimed by the caller.
    pub collateral_mint: ContentId,
    /// Realm-selected collateral token-program identity claimed by the caller.
    pub collateral_token_program: ContentId,
}

/// Project the current Source/Product route digest from an untrusted preimage.
///
/// This function is deterministic hashing only. It performs no account,
/// release, PDA, Registry, Product, Realm, or collateral authentication and
/// must never be treated as an authorization receipt by itself.
pub fn project_source_product_route_id_v4(
    projection: &SourceProductRouteIdProjectionV4,
) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_PRODUCT_ROUTE_AUTHENTICATION_DOMAIN_V4);
    hasher.update(projection.source_route_id.bytes());
    hasher.update(projection.receiver_route_id.bytes());
    hasher.update(projection.source_release_manifest_id.bytes());
    hasher.update(projection.source_release_authentication_id.bytes());
    hasher.update(projection.source_plane_contract_id.bytes());
    hasher.update(projection.source_spec_id.bytes());
    hasher.update(projection.registry_release_id.bytes());
    hasher.update(projection.capability_profile_id.bytes());
    hasher.update(projection.compiler_bundle_id.bytes());
    hasher.update(projection.market_genesis_profile_id.bytes());
    hasher.update(projection.realm_id.bytes());
    hasher.update(projection.profile_id.bytes());
    hasher.update(projection.collateral_mint.bytes());
    hasher.update(projection.collateral_token_program.bytes());
    ContentId::from_bytes(hasher.finalize().into())
}

/// Untrusted complete preimage of a current successful Source resolution input.
///
/// This record deliberately accepts only identities and runtime keys. It does
/// not prove that a successful handoff exists, that its persisted account bytes
/// are authentic, or that any field belongs to the projected Product route.
/// Those joins remain the live adapter's responsibility before projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceResolutionInputIdProjectionV4 {
    /// Already projected current Source/Product route identity.
    pub route_id: ContentId,
    /// Source policy-handoff join identity claimed by the caller.
    pub source_handoff_authentication_id: ContentId,
    /// Persisted policy-handoff authentication identity claimed by the caller.
    pub persisted_handoff_authentication_id: ContentId,
    /// Persisted policy-handoff runtime account claimed by the caller.
    pub persisted_handoff_account: RuntimeKey,
    /// Successful evaluator handoff identity claimed by the caller.
    pub successful_evaluation_handoff_id: ContentId,
    /// Compiled occurrence identity claimed by the caller.
    pub occurrence_id: ContentId,
    /// Occurrence runtime account claimed by the caller.
    pub occurrence_account: RuntimeKey,
    /// Result runtime account claimed by the caller.
    pub result_account: RuntimeKey,
    /// Complete result-account data identity claimed by the caller.
    pub result_account_data_id: ContentId,
    /// Result-account authentication identity claimed by the caller.
    pub result_account_authentication_id: ContentId,
    /// Source work-receipt authentication identity claimed by the caller.
    pub work_receipt_authentication_id: ContentId,
    /// Failure-policy binding identity claimed by the caller.
    pub failure_policy_binding_id: ContentId,
    /// Economic Market instance identity claimed by the caller.
    pub market_instance_id: ContentId,
    /// Source repair generation claimed by the caller.
    pub source_repair_generation: u64,
    /// Source Window identity claimed by the caller.
    pub window_id: ContentId,
    /// Source StatisticKey identity claimed by the caller.
    pub statistic_key_id: ContentId,
}

/// Project the current successful Source resolution-input digest.
///
/// This function is deterministic hashing only. It authenticates no handoff,
/// account, occurrence, result, work receipt, policy, Market, Window, or
/// Statistic identity and grants no permission to execute a resolution.
pub fn project_source_resolution_input_id_v4(
    projection: &SourceResolutionInputIdProjectionV4,
) -> ContentId {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_RESOLUTION_INPUT_DOMAIN_V4);
    hasher.update(projection.route_id.bytes());
    hasher.update(projection.source_handoff_authentication_id.bytes());
    hasher.update(projection.persisted_handoff_authentication_id.bytes());
    hasher.update(projection.persisted_handoff_account.bytes());
    hasher.update(projection.successful_evaluation_handoff_id.bytes());
    hasher.update(projection.occurrence_id.bytes());
    hasher.update(projection.occurrence_account.bytes());
    hasher.update(projection.result_account.bytes());
    hasher.update(projection.result_account_data_id.bytes());
    hasher.update(projection.result_account_authentication_id.bytes());
    hasher.update(projection.work_receipt_authentication_id.bytes());
    hasher.update(projection.failure_policy_binding_id.bytes());
    hasher.update(projection.market_instance_id.bytes());
    hasher.update(projection.source_repair_generation.to_le_bytes());
    hasher.update(projection.window_id.bytes());
    hasher.update(projection.statistic_key_id.bytes());
    ContentId::from_bytes(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    const fn key(byte: u8) -> RuntimeKey {
        RuntimeKey::from_bytes([byte; 32])
    }

    fn route() -> SourceProductRouteIdProjectionV4 {
        SourceProductRouteIdProjectionV4 {
            source_route_id: id(1),
            receiver_route_id: id(2),
            source_release_manifest_id: id(3),
            source_release_authentication_id: id(4),
            source_plane_contract_id: id(5),
            source_spec_id: id(6),
            registry_release_id: id(7),
            capability_profile_id: id(8),
            compiler_bundle_id: id(9),
            market_genesis_profile_id: id(10),
            realm_id: id(11),
            profile_id: id(12),
            collateral_mint: id(13),
            collateral_token_program: id(14),
        }
    }

    fn resolution(route_id: ContentId) -> SourceResolutionInputIdProjectionV4 {
        SourceResolutionInputIdProjectionV4 {
            route_id,
            source_handoff_authentication_id: id(16),
            persisted_handoff_authentication_id: id(17),
            persisted_handoff_account: key(18),
            successful_evaluation_handoff_id: id(19),
            occurrence_id: id(20),
            occurrence_account: key(21),
            result_account: key(22),
            result_account_data_id: id(23),
            result_account_authentication_id: id(24),
            work_receipt_authentication_id: id(25),
            failure_policy_binding_id: id(26),
            market_instance_id: id(27),
            source_repair_generation: 0x0102_0304_0506_0708,
            window_id: id(28),
            statistic_key_id: id(29),
        }
    }

    #[test]
    fn route_projection_commits_registry_and_collateral_edges() {
        let projection = route();
        let expected = project_source_product_route_id_v4(&projection);
        assert_eq!(
            expected.bytes(),
            [
                0x0e, 0x1a, 0xb9, 0x8a, 0x5a, 0xbb, 0x4a, 0xc9, 0x01, 0x5f, 0x99, 0x56, 0xb7, 0x98,
                0x42, 0x52, 0x2b, 0x59, 0x7e, 0xe6, 0x81, 0x48, 0x1a, 0x98, 0xa2, 0xd6, 0xb1, 0xb2,
                0x61, 0x7a, 0x1e, 0xab,
            ]
        );

        let mut different_registry = projection;
        different_registry.registry_release_id = id(42);
        assert_ne!(
            project_source_product_route_id_v4(&different_registry),
            expected
        );

        let mut different_token_program = projection;
        different_token_program.collateral_token_program = id(43);
        assert_ne!(
            project_source_product_route_id_v4(&different_token_program),
            expected
        );
    }

    #[test]
    fn resolution_projection_commits_persisted_account_and_generation() {
        let route_id = project_source_product_route_id_v4(&route());
        let projection = resolution(route_id);
        let expected = project_source_resolution_input_id_v4(&projection);
        assert_eq!(
            expected.bytes(),
            [
                0x7b, 0x36, 0x76, 0x7f, 0xbe, 0x6a, 0x0f, 0xc9, 0x82, 0xa5, 0xd9, 0x4e, 0xae, 0xae,
                0x00, 0x26, 0xa8, 0x39, 0xc5, 0x3a, 0x3e, 0x90, 0xef, 0x87, 0x85, 0x7d, 0x31, 0xac,
                0x8f, 0x16, 0x3a, 0xe2,
            ]
        );

        let mut different_account = projection;
        different_account.persisted_handoff_account = key(44);
        assert_ne!(
            project_source_resolution_input_id_v4(&different_account),
            expected
        );

        let mut different_generation = projection;
        different_generation.source_repair_generation += 1;
        assert_ne!(
            project_source_resolution_input_id_v4(&different_generation),
            expected
        );
    }
}
