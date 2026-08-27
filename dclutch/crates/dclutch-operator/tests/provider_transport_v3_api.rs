//! Compile-time compatibility checks for extracted provider and message operators.

use dclutch_operator::{
    provider_transport_v3 as compatibility, versioned as versioned_compatibility,
};
use dclutch_provider_transport_v3_operator as canonical;
use dclutch_versioned_message_operator as versioned_canonical;

#[test]
fn monolith_reexports_identical_provider_and_message_types() {
    let _submit: fn(
        &canonical::ProviderSubmitSnapshotV3,
        canonical::ProviderSubmitDeploymentV3,
        &canonical::ProviderSubmitIntentV3,
    ) -> Result<
        canonical::ProviderTransportReportV3,
        canonical::ProviderTransportOperatorErrorV3,
    > = compatibility::build_provider_submit_v3;
    let _reclaim: fn(
        &canonical::ObservedAccount,
        &canonical::ObservedAccount,
        canonical::ProviderReclaimDeploymentV3,
    ) -> Result<
        canonical::ProviderTransportReportV3,
        canonical::ProviderTransportOperatorErrorV3,
    > = compatibility::build_provider_reclaim_v3;
    let _report_identity: fn(
        compatibility::ProviderTransportReportV3,
    ) -> canonical::ProviderTransportReportV3 = |value| value;
    let _message_identity: fn(
        versioned_compatibility::VersionedMessagePlanV0,
    ) -> versioned_canonical::VersionedMessagePlanV0 = |value| value;
    let _routing_error_identity: fn(versioned_compatibility::Error) -> versioned_canonical::Error =
        |value| value;
}
