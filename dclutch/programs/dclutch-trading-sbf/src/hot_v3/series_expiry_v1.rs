//! Exact selector for the pre-Market Series expiry mode.
//!
//! This module does not recognize a request by a short magic prefix. A caller
//! enters the exceptional pre-Market path only after the Series kernel has
//! hostile-decoded the complete family request and the already-authenticated
//! ProgramSet has selected a V4 descriptor whose complete semantic/schema
//! shape is Series Expire. Every malformed request and every well-formed
//! lookalike remains on ordinary Hot's unchanged live-Market path.

extern crate alloc;

use dclutch_account_profile_contract::v3::SCHEMA_RELEASE_ID_V3 as ACCOUNT_PROFILE_SCHEMA_ID_V3;
use dclutch_capability_program_contract::v4::{
    CapabilityProgramV4, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::v5::SCHEMA_RELEASE_ID_V5 as EFFECT_SCHEMA_ID_V5;
use dclutch_execution_strategy_contract::v2::EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2;
use dclutch_request_profile_contract::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1;
use dclutch_series_v3_kernel::{
    generated::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    replay::SERIES_STATE_BYTES_V3,
    request::{SeriesActionRequestV3, SeriesActionV3},
};
use dclutch_transition_vm::v3::SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_ID_V3;
use solana_program::hash::hash;

use crate::series::artifacts_v3::{
    SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ROOT_SCHEMA_PREIMAGE_V3,
    SERIES_SUCCESSOR_KIND_PREIMAGE_V3, SERIES_TICKET_DERIVATION_PREIMAGE_V3,
};

pub(super) const SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1: usize = 81;
pub(super) const SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1: usize = 5;
pub(super) const SERIES_EXPIRE_CORE_ROUTE_START_V1: usize = 55;
pub(super) const SERIES_EXPIRE_CORE_ROUTE_COUNT_V1: usize = 26;
/// Vacant future Market carried by projected-Custody Abort, never the fixed
/// live controller Market authenticated by ordinary Hot.
pub(super) const SERIES_EXPIRE_FUTURE_MARKET_ACCOUNT_V1: usize = 54;
pub(super) const SERIES_EXPIRE_PERMIT_ACCOUNT_V1: usize = SERIES_EXPIRE_CORE_ROUTE_START_V1;
pub(super) const SERIES_EXPIRE_RENT_CREDIT_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 1;
pub(super) const SERIES_EXPIRE_RENT_PROGRAM_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 2;
pub(super) const SERIES_EXPIRE_ROOT_REPLAY_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 14;
pub(super) const SERIES_EXPIRE_TICKET_REPLAY_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 15;
pub(super) const SERIES_EXPIRE_TEMPLATE_RAW_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 16;
pub(super) const SERIES_EXPIRE_TEMPLATE_STAGING_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 17;
pub(super) const SERIES_EXPIRE_OCCURRENCE_RAW_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 18;
pub(super) const SERIES_EXPIRE_OCCURRENCE_STAGING_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 19;
pub(super) const SERIES_EXPIRE_TICKET_RAW_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 20;
pub(super) const SERIES_EXPIRE_TICKET_STAGING_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 21;
pub(super) const SERIES_EXPIRE_SYSTEM_PROGRAM_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 24;
const SERIES_EXPIRE_CALLER_ACCOUNT_V1: usize = SERIES_EXPIRE_CORE_ROUTE_START_V1 + 25;

const _: () = {
    assert!(SERIES_EXPIRE_CORE_ROUTE_START_V1 + SERIES_EXPIRE_CORE_ROUTE_COUNT_V1 == 81);
    assert!(SERIES_EXPIRE_CALLER_ACCOUNT_V1 + 1 == SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1);
    assert!(SERIES_EXPIRE_ROOT_REPLAY_ACCOUNT_V1 == 69);
    assert!(SERIES_EXPIRE_TICKET_REPLAY_ACCOUNT_V1 == 70);
    assert!(SERIES_EXPIRE_FUTURE_MARKET_ACCOUNT_V1 + 1 == SERIES_EXPIRE_CORE_ROUTE_START_V1);
    assert!(SERIES_EXPIRE_OCCURRENCE_RAW_ACCOUNT_V1 == 73);
    assert!(SERIES_EXPIRE_TICKET_RAW_ACCOUNT_V1 == 75);
};

/// A request which has not yet earned exceptional pre-Market behavior.
///
/// `None` is intentionally the only negative result: classifier failure is not
/// a protocol refusal. The caller must continue through ordinary Hot so short,
/// malformed, or merely Series-looking bytes retain its historical outcome.
pub(super) fn classify_selected_series_expiry_v1(
    family_request: &[u8],
    selected_action: u32,
    selected_config: ContentId,
    descriptor: CapabilityProgramV4,
) -> Option<SeriesActionRequestV3<'_>> {
    let request = SeriesActionRequestV3::decode(family_request).ok()?;
    if request.action() != SeriesActionV3::Expire
        || selected_action != SeriesActionV3::Expire as u32
        || request.template() != selected_config
        || !is_exact_series_expiry_descriptor_v1(descriptor)
    {
        return None;
    }
    Some(request)
}

fn is_exact_series_expiry_descriptor_v1(descriptor: CapabilityProgramV4) -> bool {
    descriptor.kind().to_bytes() == hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes()
        && descriptor.config_schema().to_bytes() == SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3
        && descriptor.request_schema().to_bytes()
            == hash(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).to_bytes()
        && descriptor.root_schema().to_bytes() == hash(SERIES_ROOT_SCHEMA_PREIMAGE_V3).to_bytes()
        && descriptor.derivation_policy().to_bytes()
            == hash(SERIES_TICKET_DERIVATION_PREIMAGE_V3).to_bytes()
        && descriptor.account_profile().schema().to_bytes() == ACCOUNT_PROFILE_SCHEMA_ID_V3
        && descriptor.request_profile().schema().to_bytes() == REQUEST_PROFILE_SCHEMA_ID_V1
        && descriptor.lifecycle().schema().to_bytes() == SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5
        && descriptor.strategy().schema().to_bytes() == EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        && descriptor.transition().schema().to_bytes() == TRANSITION_SCHEMA_ID_V3
        && descriptor.effect().schema().to_bytes() == EFFECT_SCHEMA_ID_V5
        && usize::try_from(descriptor.root_state_bytes()).ok() == Some(SERIES_STATE_BYTES_V3)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use dclutch_capability_program_contract::v4::{ArtifactReferenceV4, CapabilityArtifactsV4};
    use dclutch_series_v3_kernel::request::encode_series_action_header_v3;

    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero content identity")
    }

    fn reference(schema: [u8; 32], program: u8) -> ArtifactReferenceV4 {
        ArtifactReferenceV4::new(ContentId::new(schema).expect("nonzero schema"), id(program))
    }

    fn exact_descriptor() -> CapabilityProgramV4 {
        CapabilityProgramV4::new(
            ContentId::new(hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes())
                .expect("Series kind"),
            ContentId::new(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3).expect("Template schema"),
            ContentId::new(hash(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).to_bytes())
                .expect("request schema"),
            ContentId::new(hash(SERIES_ROOT_SCHEMA_PREIMAGE_V3).to_bytes()).expect("root schema"),
            ContentId::new(hash(SERIES_TICKET_DERIVATION_PREIMAGE_V3).to_bytes())
                .expect("Ticket derivation"),
            id(0x20),
            CapabilityArtifactsV4 {
                account_profile: reference(ACCOUNT_PROFILE_SCHEMA_ID_V3, 0x21),
                request_profile: reference(REQUEST_PROFILE_SCHEMA_ID_V1, 0x22),
                lifecycle: reference(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, 0x23),
                strategy: reference(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, 0x24),
                transition: reference(TRANSITION_SCHEMA_ID_V3, 0x25),
                effect: reference(EFFECT_SCHEMA_ID_V5, 0x26),
            },
            u32::try_from(SERIES_STATE_BYTES_V3).expect("Series state width"),
        )
        .expect("exact descriptor")
    }

    fn request(action: SeriesActionV3, template: ContentId) -> Vec<u8> {
        let header = encode_series_action_header_v3(
            action,
            template,
            Some(id(0x31)),
            Some(id(0x32)),
            7,
            3,
            1,
        )
        .expect("family header");
        let mut output = header.to_vec();
        output.extend_from_slice(&[0x44; 32]);
        output
    }

    #[test]
    fn exact_hostile_decode_and_descriptor_are_both_required() {
        let template = id(0x30);
        let family = request(SeriesActionV3::Expire, template);
        let selected = classify_selected_series_expiry_v1(
            &family,
            SeriesActionV3::Expire as u32,
            template,
            exact_descriptor(),
        )
        .expect("exact pre-Market Series expiry");
        assert_eq!(selected.bytes(), family);
        assert_eq!(selected.proof_bytes(), [0x44; 32]);
    }

    #[test]
    fn malformed_and_series_lookalike_requests_do_not_select() {
        let template = id(0x30);
        let exact = request(SeriesActionV3::Expire, template);
        assert!(
            classify_selected_series_expiry_v1(
                exact.get(..exact.len() - 1).expect("short request"),
                SeriesActionV3::Expire as u32,
                template,
                exact_descriptor(),
            )
            .is_none()
        );

        let consume = request(SeriesActionV3::Consume, template);
        assert!(
            classify_selected_series_expiry_v1(
                &consume,
                SeriesActionV3::Consume as u32,
                template,
                exact_descriptor(),
            )
            .is_none()
        );

        assert!(
            classify_selected_series_expiry_v1(
                &exact,
                SeriesActionV3::Expire as u32,
                id(0x77),
                exact_descriptor(),
            )
            .is_none()
        );
    }

    #[test]
    fn schema_substitution_does_not_earn_the_exception() {
        let template = id(0x30);
        let family = request(SeriesActionV3::Expire, template);
        let mut descriptor = exact_descriptor();
        let mut bytes = descriptor.encode();
        // Hostile-decode a different but individually valid account-profile
        // schema. It must not be enough that the descriptor is still V4.
        let replacement = [0x7a; 32];
        let offset = dclutch_capability_program_contract::v4::CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET;
        bytes[offset..offset + 32].copy_from_slice(&replacement);
        descriptor = CapabilityProgramV4::decode(&bytes).expect("valid substituted descriptor");
        assert!(
            classify_selected_series_expiry_v1(
                &family,
                SeriesActionV3::Expire as u32,
                template,
                descriptor,
            )
            .is_none()
        );
    }
}
