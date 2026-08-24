//! Durable Source terminal policy for Source-owned failure handoffs.

use clutch_source_plane_v3::{ContentId, FixedCodec};
use clutch_source_plane_v3_adapter::PdaRecipeV3;

use crate::auth::{
    account_data_id, domain_id, AuthenticatedSourceRouteV1, RuntimeAccountViewV1,
    RuntimeDerivedPdaV1, RuntimeKey,
};
use crate::lineage::{AuthenticatedReopenLineageV1, LineageAccessV1, LineageFamilyV1};
use crate::window::SourceFailureKindV1;
use crate::{Error, Result};

const SOURCE_FAILURE_TERMINAL_MAGIC: [u8; 8] = *b"DCSPFT01";
const SOURCE_FAILURE_TERMINAL_DOMAIN: &[u8] =
    b"dragons-clutch/source-failure-terminal/v1";
const SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_MAGIC: [u8; 8] = *b"DCSPFB02";
const SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_DOMAIN: &[u8] =
    b"dragons-clutch/source-failure-terminal-product-release/v2";
const SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_AUTH_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-source-failure-terminal-product-release/v2";

/// Exact fixed width of one Source failure-terminal record.
pub const SOURCE_FAILURE_TERMINAL_BYTES: usize = 672;
/// Exact fixed width of the current per-occurrence terminal/release owner.
pub const SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_BYTES: usize = 1184;

/// One-way post-terminal Product-release binding phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFailureTerminalProductReleasePhaseV2 {
    /// Source terminal exists, but Product has not released the pinned Link.
    PendingProductRelease,
    /// Exact Source/Product release bridge is durably bound once.
    BoundProductRelease,
}

impl SourceFailureTerminalProductReleasePhaseV2 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::PendingProductRelease => 1,
            Self::BoundProductRelease => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::PendingProductRelease),
            2 => Ok(Self::BoundProductRelease),
            _ => Err(Error::InvalidCodec),
        }
    }
}

/// Source-owned exhaustive Product release disposition for a failed attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFailureProductReleaseDispositionV2 {
    /// Product released the pinned link because no accepted Result existed.
    SourceAbsent,
    /// Product released the pinned link because the persisted Result refused.
    SourceRefused,
}

impl SourceFailureProductReleaseDispositionV2 {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::SourceAbsent => 1,
            Self::SourceRefused => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::SourceAbsent),
            2 => Ok(Self::SourceRefused),
            _ => Err(Error::InvalidCodec),
        }
    }
}

/// Exhaustive physical terminal disposition for a Source-owned failure fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFailureTerminalDispositionV1 {
    /// No Result account exists; permanently retire the never-created lineage.
    AbsenceLineageTombstone,
    /// A refused Result exists; close it and its open lineage without reopen.
    RefusedResultClose,
}

impl SourceFailureTerminalDispositionV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::AbsenceLineageTombstone => 1,
            Self::RefusedResultClose => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::AbsenceLineageTombstone),
            2 => Ok(Self::RefusedResultClose),
            _ => Err(Error::InvalidCodec),
        }
    }
}

/// Immutable Source-owned decision written only after Failure's exact final
/// zero-payout/source-failure postwrite authenticates the reconstructed handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceFailureTerminalV1 {
    source_release_manifest_id: ContentId,
    source_release_authentication_id: ContentId,
    route_id: ContentId,
    source_plane_contract_id: ContentId,
    source_spec_id: ContentId,
    source_work_schedule_id: ContentId,
    source_reconstruction_id: ContentId,
    source_handoff_id: ContentId,
    source_handoff_join_id: ContentId,
    persisted_handoff_authentication_id: ContentId,
    source_failure_terminal_authority_id: ContentId,
    market_instance_id: ContentId,
    failure_policy_binding_id: ContentId,
    source_fact_authentication_id: ContentId,
    statistic_key_id: ContentId,
    lineage_authentication_id: ContentId,
    expected_lineage_state_id: ContentId,
    lineage_recipe_id: ContentId,
    lineage_account: RuntimeKey,
    result_or_absence_account: RuntimeKey,
    failure_generation: u64,
    source_repair_generation: u64,
    source_failure_kind: SourceFailureKindV1,
    disposition: SourceFailureTerminalDispositionV1,
}

impl SourceFailureTerminalV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        route: AuthenticatedSourceRouteV1,
        source_reconstruction_id: ContentId,
        source_handoff_id: ContentId,
        source_handoff_join_id: ContentId,
        persisted_handoff_authentication_id: ContentId,
        source_failure_terminal_authority_id: ContentId,
        market_instance_id: ContentId,
        failure_policy_binding_id: ContentId,
        source_fact_authentication_id: ContentId,
        statistic_key_id: ContentId,
        lineage: AuthenticatedReopenLineageV1,
        result_or_absence_account: RuntimeKey,
        failure_generation: u64,
        source_repair_generation: u64,
        source_failure_kind: SourceFailureKindV1,
        disposition: SourceFailureTerminalDispositionV1,
    ) -> Result<Self> {
        let state = lineage.lineage();
        let statistic_result_recipe_id = PdaRecipeV3::statistic_result(statistic_key_id)?.id()?;
        if lineage.access() != LineageAccessV1::Mutable
            || state.family != LineageFamilyV1::StatisticResult
            || state.semantic_binding_id != statistic_result_recipe_id
        {
            return Err(Error::InvalidLineage);
        }
        match (source_failure_kind, disposition) {
            (
                SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution,
                SourceFailureTerminalDispositionV1::AbsenceLineageTombstone,
            ) => {
                if state.latest_generation != 0
                    || state.is_open
                    || !state.active_account.is_zero()
                    || !state.last_opened_state_id.is_zero()
                    || !state.last_close_receipt_id.is_zero()
                {
                    return Err(Error::InvalidLineage);
                }
            }
            (
                SourceFailureKindV1::SourceEvaluationRefused,
                SourceFailureTerminalDispositionV1::RefusedResultClose,
            ) => {
                if !state.is_open || state.active_account != result_or_absence_account {
                    return Err(Error::InvalidLineage);
                }
            }
            _ => return Err(Error::MismatchedBinding),
        }
        let value = Self {
            source_release_manifest_id: route.release_manifest_id(),
            source_release_authentication_id: route.release_authentication_id(),
            route_id: route.route_id(),
            source_plane_contract_id: route.source_plane_contract_id(),
            source_spec_id: route.source_spec_id(),
            source_work_schedule_id: route.source_work_schedule_id(),
            source_reconstruction_id,
            source_handoff_id,
            source_handoff_join_id,
            persisted_handoff_authentication_id,
            source_failure_terminal_authority_id,
            market_instance_id,
            failure_policy_binding_id,
            source_fact_authentication_id,
            statistic_key_id,
            lineage_authentication_id: lineage.id(),
            expected_lineage_state_id: lineage.account_data_id(),
            lineage_recipe_id: state.recipe_id()?,
            lineage_account: state.lineage_account,
            result_or_absence_account,
            failure_generation,
            source_repair_generation,
            source_failure_kind,
            disposition,
        };
        value.validate_shape()?;
        Ok(value)
    }

    fn validate_shape(&self) -> Result<()> {
        for id in [
            self.source_release_manifest_id,
            self.source_release_authentication_id,
            self.route_id,
            self.source_plane_contract_id,
            self.source_spec_id,
            self.source_work_schedule_id,
            self.source_reconstruction_id,
            self.source_handoff_id,
            self.source_handoff_join_id,
            self.persisted_handoff_authentication_id,
            self.source_failure_terminal_authority_id,
            self.market_instance_id,
            self.failure_policy_binding_id,
            self.source_fact_authentication_id,
            self.statistic_key_id,
            self.lineage_authentication_id,
            self.expected_lineage_state_id,
            self.lineage_recipe_id,
        ] {
            if id.is_zero() {
                return Err(Error::ZeroIdentity);
            }
        }
        self.lineage_account.validate()?;
        self.result_or_absence_account.validate()?;
        if self.failure_generation == 0 {
            return Err(Error::MismatchedBinding);
        }
        match (self.source_failure_kind, self.disposition) {
            (
                SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution,
                SourceFailureTerminalDispositionV1::AbsenceLineageTombstone,
            )
            | (
                SourceFailureKindV1::SourceEvaluationRefused,
                SourceFailureTerminalDispositionV1::RefusedResultClose,
            ) => Ok(()),
            _ => Err(Error::MismatchedBinding),
        }
    }

    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0_u8; SOURCE_FAILURE_TERMINAL_BYTES];
        self.encode_into(&mut bytes).map_err(Error::Core)?;
        Ok(domain_id(SOURCE_FAILURE_TERMINAL_DOMAIN, &bytes))
    }

    pub const fn source_reconstruction_id(&self) -> ContentId {
        self.source_reconstruction_id
    }

    pub const fn source_handoff_id(&self) -> ContentId {
        self.source_handoff_id
    }

    pub const fn source_handoff_join_id(&self) -> ContentId {
        self.source_handoff_join_id
    }

    pub const fn persisted_handoff_authentication_id(&self) -> ContentId {
        self.persisted_handoff_authentication_id
    }

    /// Exact post-Product-pin, pre-Failure-cell authority authenticated before
    /// this Source terminal was persisted.
    pub const fn source_failure_terminal_authority_id(&self) -> ContentId {
        self.source_failure_terminal_authority_id
    }

    pub const fn lineage_authentication_id(&self) -> ContentId {
        self.lineage_authentication_id
    }

    pub const fn expected_lineage_state_id(&self) -> ContentId {
        self.expected_lineage_state_id
    }

    pub const fn lineage_account(&self) -> RuntimeKey {
        self.lineage_account
    }

    pub const fn result_or_absence_account(&self) -> RuntimeKey {
        self.result_or_absence_account
    }

    pub const fn statistic_key_id(&self) -> ContentId {
        self.statistic_key_id
    }

    pub const fn source_failure_kind(&self) -> SourceFailureKindV1 {
        self.source_failure_kind
    }

    pub const fn disposition(&self) -> SourceFailureTerminalDispositionV1 {
        self.disposition
    }
}

impl FixedCodec for SourceFailureTerminalV1 {
    const ENCODED_LEN: usize = SOURCE_FAILURE_TERMINAL_BYTES;

    fn encode_into(
        &self,
        output: &mut [u8],
    ) -> core::result::Result<(), clutch_source_plane_v3::Error> {
        if output.len() < Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::Truncated);
        }
        if output.len() > Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::TrailingBytes);
        }
        self.validate_shape()
            .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?;
        output.fill(0);
        output[..8].copy_from_slice(&SOURCE_FAILURE_TERMINAL_MAGIC);
        output[8..10].copy_from_slice(&1_u16.to_le_bytes());
        output[10] = failure_kind_byte(self.source_failure_kind);
        output[11] = self.disposition.byte();
        let values = [
            self.source_release_manifest_id.bytes(),
            self.source_release_authentication_id.bytes(),
            self.route_id.bytes(),
            self.source_plane_contract_id.bytes(),
            self.source_spec_id.bytes(),
            self.source_work_schedule_id.bytes(),
            self.source_reconstruction_id.bytes(),
            self.source_handoff_id.bytes(),
            self.source_handoff_join_id.bytes(),
            self.persisted_handoff_authentication_id.bytes(),
            self.source_failure_terminal_authority_id.bytes(),
            self.market_instance_id.bytes(),
            self.failure_policy_binding_id.bytes(),
            self.source_fact_authentication_id.bytes(),
            self.statistic_key_id.bytes(),
            self.lineage_authentication_id.bytes(),
            self.expected_lineage_state_id.bytes(),
            self.lineage_recipe_id.bytes(),
            self.lineage_account.bytes(),
            self.result_or_absence_account.bytes(),
        ];
        let mut at = 16_usize;
        for value in values {
            output[at..at + 32].copy_from_slice(&value);
            at += 32;
        }
        output[656..664].copy_from_slice(&self.failure_generation.to_le_bytes());
        output[664..672].copy_from_slice(&self.source_repair_generation.to_le_bytes());
        Ok(())
    }

    fn decode(input: &[u8]) -> core::result::Result<Self, clutch_source_plane_v3::Error> {
        if input.len() < Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::Truncated);
        }
        if input.len() > Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::TrailingBytes);
        }
        if input[..8] != SOURCE_FAILURE_TERMINAL_MAGIC {
            return Err(clutch_source_plane_v3::Error::BadMagic);
        }
        if input[8..10] != 1_u16.to_le_bytes() {
            return Err(clutch_source_plane_v3::Error::BadVersion);
        }
        if input[12..16].iter().any(|byte| *byte != 0) {
            return Err(clutch_source_plane_v3::Error::NonCanonicalReserved);
        }
        let read_32 = |at: usize| {
            let mut value = [0_u8; 32];
            value.copy_from_slice(&input[at..at + 32]);
            value
        };
        let read_u64 = |at: usize| {
            let mut value = [0_u8; 8];
            value.copy_from_slice(&input[at..at + 8]);
            u64::from_le_bytes(value)
        };
        let values = Self {
            source_release_manifest_id: ContentId::from_bytes(read_32(16)),
            source_release_authentication_id: ContentId::from_bytes(read_32(48)),
            route_id: ContentId::from_bytes(read_32(80)),
            source_plane_contract_id: ContentId::from_bytes(read_32(112)),
            source_spec_id: ContentId::from_bytes(read_32(144)),
            source_work_schedule_id: ContentId::from_bytes(read_32(176)),
            source_reconstruction_id: ContentId::from_bytes(read_32(208)),
            source_handoff_id: ContentId::from_bytes(read_32(240)),
            source_handoff_join_id: ContentId::from_bytes(read_32(272)),
            persisted_handoff_authentication_id: ContentId::from_bytes(read_32(304)),
            source_failure_terminal_authority_id: ContentId::from_bytes(read_32(336)),
            market_instance_id: ContentId::from_bytes(read_32(368)),
            failure_policy_binding_id: ContentId::from_bytes(read_32(400)),
            source_fact_authentication_id: ContentId::from_bytes(read_32(432)),
            statistic_key_id: ContentId::from_bytes(read_32(464)),
            lineage_authentication_id: ContentId::from_bytes(read_32(496)),
            expected_lineage_state_id: ContentId::from_bytes(read_32(528)),
            lineage_recipe_id: ContentId::from_bytes(read_32(560)),
            lineage_account: RuntimeKey::from_bytes(read_32(592)),
            result_or_absence_account: RuntimeKey::from_bytes(read_32(624)),
            failure_generation: read_u64(656),
            source_repair_generation: read_u64(664),
            source_failure_kind: decode_failure_kind(input[10])
                .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?,
            disposition: SourceFailureTerminalDispositionV1::decode(input[11])
                .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?,
        };
        values
            .validate_shape()
            .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?;
        Ok(values)
    }
}

/// Current per-occurrence Source terminal owner. The inner terminal remains
/// immutable while the exact post-Product-release evidence is bound once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceFailureTerminalAccountV2 {
    phase: SourceFailureTerminalProductReleasePhaseV2,
    disposition: Option<SourceFailureProductReleaseDispositionV2>,
    terminal: SourceFailureTerminalV1,
    product_release_binding_id: ContentId,
    product_release_facts_id: ContentId,
    product_release_id: ContentId,
    product_link_account: RuntimeKey,
    product_link_authentication_before: ContentId,
    product_link_authentication_after: ContentId,
    product_link_semantic_before: ContentId,
    product_link_semantic_after: ContentId,
    product_transition_sequence_before: u64,
    product_transition_sequence_after: u64,
    product_session_transcript_before: ContentId,
    product_session_transcript_after: ContentId,
    product_session_terminal_receipt_id: ContentId,
    product_archive_postwrite_id: ContentId,
    product_append_receipt_id: ContentId,
    product_reset_receipt_id: ContentId,
    product_release_preauthorization_id: ContentId,
}

impl SourceFailureTerminalAccountV2 {
    /// Create the exact prefunded pending body before Product can release.
    pub fn new_pending(terminal: SourceFailureTerminalV1) -> Result<Self> {
        terminal.validate_shape()?;
        let value = Self {
            phase: SourceFailureTerminalProductReleasePhaseV2::PendingProductRelease,
            disposition: None,
            terminal,
            product_release_binding_id: ContentId::ZERO,
            product_release_facts_id: ContentId::ZERO,
            product_release_id: ContentId::ZERO,
            product_link_account: RuntimeKey::from_bytes([0; 32]),
            product_link_authentication_before: ContentId::ZERO,
            product_link_authentication_after: ContentId::ZERO,
            product_link_semantic_before: ContentId::ZERO,
            product_link_semantic_after: ContentId::ZERO,
            product_transition_sequence_before: 0,
            product_transition_sequence_after: 0,
            product_session_transcript_before: ContentId::ZERO,
            product_session_transcript_after: ContentId::ZERO,
            product_session_terminal_receipt_id: ContentId::ZERO,
            product_archive_postwrite_id: ContentId::ZERO,
            product_append_receipt_id: ContentId::ZERO,
            product_reset_receipt_id: ContentId::ZERO,
            product_release_preauthorization_id: ContentId::ZERO,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Bind the exact post-release bridge once without changing the inner
    /// terminal semantic identity or accepting any later caller projection.
    #[allow(clippy::too_many_arguments)]
    pub fn bind_product_release(
        self,
        disposition: SourceFailureProductReleaseDispositionV2,
        product_release_binding_id: ContentId,
        product_release_facts_id: ContentId,
        product_release_id: ContentId,
        product_link_account: RuntimeKey,
        product_link_authentication_before: ContentId,
        product_link_authentication_after: ContentId,
        product_link_semantic_before: ContentId,
        product_link_semantic_after: ContentId,
        product_transition_sequence_before: u64,
        product_transition_sequence_after: u64,
        product_session_transcript_before: ContentId,
        product_session_transcript_after: ContentId,
        product_session_terminal_receipt_id: ContentId,
        product_archive_postwrite_id: ContentId,
        product_append_receipt_id: ContentId,
        product_reset_receipt_id: ContentId,
        product_release_preauthorization_id: ContentId,
    ) -> Result<Self> {
        self.validate_shape()?;
        if self.phase != SourceFailureTerminalProductReleasePhaseV2::PendingProductRelease
            || self.disposition.is_some()
            || !self.product_release_binding_id.is_zero()
            || !self.product_release_facts_id.is_zero()
            || !self.product_release_id.is_zero()
            || !self.product_link_account.is_zero()
            || !self.product_link_authentication_before.is_zero()
            || !self.product_link_authentication_after.is_zero()
            || !self.product_link_semantic_before.is_zero()
            || !self.product_link_semantic_after.is_zero()
            || self.product_transition_sequence_before != 0
            || self.product_transition_sequence_after != 0
            || !self.product_session_transcript_before.is_zero()
            || !self.product_session_transcript_after.is_zero()
            || !self.product_session_terminal_receipt_id.is_zero()
            || !self.product_archive_postwrite_id.is_zero()
            || !self.product_append_receipt_id.is_zero()
            || !self.product_reset_receipt_id.is_zero()
            || !self.product_release_preauthorization_id.is_zero()
        {
            return Err(Error::MismatchedBinding);
        }
        let expected = match self.terminal.source_failure_kind() {
            SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => {
                SourceFailureProductReleaseDispositionV2::SourceAbsent
            }
            SourceFailureKindV1::SourceEvaluationRefused => {
                SourceFailureProductReleaseDispositionV2::SourceRefused
            }
        };
        if disposition != expected {
            return Err(Error::MismatchedBinding);
        }
        let value = Self {
            phase: SourceFailureTerminalProductReleasePhaseV2::BoundProductRelease,
            disposition: Some(disposition),
            product_release_binding_id,
            product_release_facts_id,
            product_release_id,
            product_link_account,
            product_link_authentication_before,
            product_link_authentication_after,
            product_link_semantic_before,
            product_link_semantic_after,
            product_transition_sequence_before,
            product_transition_sequence_after,
            product_session_transcript_before,
            product_session_transcript_after,
            product_session_terminal_receipt_id,
            product_archive_postwrite_id,
            product_append_receipt_id,
            product_reset_receipt_id,
            product_release_preauthorization_id,
            ..self
        };
        value.validate_shape()?;
        Ok(value)
    }

    fn validate_shape(&self) -> Result<()> {
        self.terminal.validate_shape()?;
        match (self.phase, self.disposition) {
            (
                SourceFailureTerminalProductReleasePhaseV2::PendingProductRelease,
                None,
            ) => {
                if !self.product_release_binding_id.is_zero()
                    || !self.product_release_facts_id.is_zero()
                    || !self.product_release_id.is_zero()
                    || !self.product_link_account.is_zero()
                    || !self.product_link_authentication_before.is_zero()
                    || !self.product_link_authentication_after.is_zero()
                    || !self.product_link_semantic_before.is_zero()
                    || !self.product_link_semantic_after.is_zero()
                    || self.product_transition_sequence_before != 0
                    || self.product_transition_sequence_after != 0
                    || !self.product_session_transcript_before.is_zero()
                    || !self.product_session_transcript_after.is_zero()
                    || !self.product_session_terminal_receipt_id.is_zero()
                    || !self.product_archive_postwrite_id.is_zero()
                    || !self.product_append_receipt_id.is_zero()
                    || !self.product_reset_receipt_id.is_zero()
                    || !self.product_release_preauthorization_id.is_zero()
                {
                    return Err(Error::MismatchedBinding);
                }
            }
            (
                SourceFailureTerminalProductReleasePhaseV2::BoundProductRelease,
                Some(disposition),
            ) => {
                let expected = match self.terminal.source_failure_kind() {
                    SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => {
                        SourceFailureProductReleaseDispositionV2::SourceAbsent
                    }
                    SourceFailureKindV1::SourceEvaluationRefused => {
                        SourceFailureProductReleaseDispositionV2::SourceRefused
                    }
                };
                if disposition != expected {
                    return Err(Error::MismatchedBinding);
                }
                self.product_link_account.validate()?;
                let ids = [
                    self.product_release_binding_id,
                    self.product_release_facts_id,
                    self.product_release_id,
                    self.product_link_authentication_before,
                    self.product_link_authentication_after,
                    self.product_link_semantic_before,
                    self.product_link_semantic_after,
                    self.product_session_transcript_before,
                    self.product_session_transcript_after,
                    self.product_session_terminal_receipt_id,
                    self.product_archive_postwrite_id,
                    self.product_append_receipt_id,
                    self.product_reset_receipt_id,
                    self.product_release_preauthorization_id,
                ];
                let mut index = 0usize;
                while index < ids.len() {
                    if ids[index].is_zero() {
                        return Err(Error::ZeroIdentity);
                    }
                    let mut prior = 0usize;
                    while prior < index {
                        if ids[prior] == ids[index] {
                            return Err(Error::IdentityAlias);
                        }
                        prior += 1;
                    }
                    index += 1;
                }
                if self.product_link_account == self.terminal.lineage_account()
                    || self.product_link_account == self.terminal.result_or_absence_account()
                    || self.product_link_authentication_before
                        == self.product_link_authentication_after
                    || self.product_link_semantic_before == self.product_link_semantic_after
                    || self.product_session_transcript_before
                        == self.product_session_transcript_after
                    || self.product_transition_sequence_after
                        != self
                            .product_transition_sequence_before
                            .checked_add(1)
                            .ok_or(Error::ArithmeticOverflow)?
                {
                    return Err(Error::IdentityAlias);
                }
            }
            _ => return Err(Error::MismatchedBinding),
        }
        Ok(())
    }

    /// One-way binding phase.
    pub const fn phase(self) -> SourceFailureTerminalProductReleasePhaseV2 {
        self.phase
    }

    /// Immutable inner Source terminal.
    pub const fn terminal(self) -> SourceFailureTerminalV1 {
        self.terminal
    }

    /// Exhaustive bound release disposition, or none only while pending.
    pub const fn disposition(self) -> Option<SourceFailureProductReleaseDispositionV2> {
        self.disposition
    }

    /// Source/Product release bridge identity.
    pub const fn product_release_binding_id(self) -> ContentId {
        self.product_release_binding_id
    }

    /// Complete bridge-facts identity.
    pub const fn product_release_facts_id(self) -> ContentId {
        self.product_release_facts_id
    }

    /// Exact Product release postwrite.
    pub const fn product_release_id(self) -> ContentId {
        self.product_release_id
    }

    /// Physical Product Link account.
    pub const fn product_link_account(self) -> RuntimeKey {
        self.product_link_account
    }

    /// Hostile Product Link authentication immediately before release.
    pub const fn product_link_authentication_before(self) -> ContentId {
        self.product_link_authentication_before
    }

    /// Hostile Product Link authentication immediately after release.
    pub const fn product_link_authentication_after(self) -> ContentId {
        self.product_link_authentication_after
    }

    /// Product Link semantic immediately before release.
    pub const fn product_link_semantic_before(self) -> ContentId {
        self.product_link_semantic_before
    }

    /// Product Link semantic immediately after release.
    pub const fn product_link_semantic_after(self) -> ContentId {
        self.product_link_semantic_after
    }

    /// Monotone Product Link transition sequence immediately before release.
    pub const fn product_transition_sequence_before(self) -> u64 {
        self.product_transition_sequence_before
    }

    /// Monotone Product Link transition sequence immediately after release.
    pub const fn product_transition_sequence_after(self) -> u64 {
        self.product_transition_sequence_after
    }

    /// Monotone Failure-session transcript immediately before release.
    pub const fn product_session_transcript_before(self) -> ContentId {
        self.product_session_transcript_before
    }

    /// Monotone Failure-session transcript immediately after release.
    pub const fn product_session_transcript_after(self) -> ContentId {
        self.product_session_transcript_after
    }

    /// Exact Product session terminal receipt consumed by release.
    pub const fn product_session_terminal_receipt_id(self) -> ContentId {
        self.product_session_terminal_receipt_id
    }

    /// Exact Failure archive postwrite consumed by release.
    pub const fn product_archive_postwrite_id(self) -> ContentId {
        self.product_archive_postwrite_id
    }

    /// Exact Product session append receipt consumed by release.
    pub const fn product_append_receipt_id(self) -> ContentId {
        self.product_append_receipt_id
    }

    /// Exact Product session reset receipt consumed by release.
    pub const fn product_reset_receipt_id(self) -> ContentId {
        self.product_reset_receipt_id
    }

    /// Exact Product release preauthorization consumed by release.
    pub const fn product_release_preauthorization_id(self) -> ContentId {
        self.product_release_preauthorization_id
    }

    /// Identity of the complete pending or bound account body.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0_u8; SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_BYTES];
        self.encode_into(&mut bytes).map_err(Error::Core)?;
        Ok(domain_id(SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_DOMAIN, &bytes))
    }
}

impl FixedCodec for SourceFailureTerminalAccountV2 {
    const ENCODED_LEN: usize = SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_BYTES;

    fn encode_into(
        &self,
        output: &mut [u8],
    ) -> core::result::Result<(), clutch_source_plane_v3::Error> {
        if output.len() < Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::Truncated);
        }
        if output.len() > Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::TrailingBytes);
        }
        self.validate_shape()
            .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?;
        output.fill(0);
        output[..8].copy_from_slice(&SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_MAGIC);
        output[8] = self.phase.wire_byte();
        output[9] = self.disposition.map_or(0, |value| value.wire_byte());
        self.terminal.encode_into(&mut output[16..688])?;
        let ids = [
            self.product_release_binding_id,
            self.product_release_facts_id,
            self.product_release_id,
            ContentId::from_bytes(self.product_link_account.bytes()),
            self.product_link_authentication_before,
            self.product_link_authentication_after,
            self.product_link_semantic_before,
            self.product_link_semantic_after,
            self.product_session_transcript_before,
            self.product_session_transcript_after,
            self.product_session_terminal_receipt_id,
            self.product_archive_postwrite_id,
            self.product_append_receipt_id,
            self.product_reset_receipt_id,
            self.product_release_preauthorization_id,
        ];
        let mut at = 688usize;
        for id in ids {
            output[at..at + 32].copy_from_slice(&id.bytes());
            at += 32;
        }
        output[1168..1176].copy_from_slice(&self.product_transition_sequence_before.to_le_bytes());
        output[1176..1184].copy_from_slice(&self.product_transition_sequence_after.to_le_bytes());
        Ok(())
    }

    fn decode(input: &[u8]) -> core::result::Result<Self, clutch_source_plane_v3::Error> {
        if input.len() < Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::Truncated);
        }
        if input.len() > Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::TrailingBytes);
        }
        if input[..8] != SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_MAGIC {
            return Err(clutch_source_plane_v3::Error::BadMagic);
        }
        if input[10..16].iter().any(|byte| *byte != 0) {
            return Err(clutch_source_plane_v3::Error::NonCanonicalReserved);
        }
        let read_32 = |at: usize| {
            let mut value = [0_u8; 32];
            value.copy_from_slice(&input[at..at + 32]);
            value
        };
        let phase = SourceFailureTerminalProductReleasePhaseV2::decode(input[8])
            .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?;
        let disposition = match input[9] {
            0 => None,
            value => Some(
                SourceFailureProductReleaseDispositionV2::decode(value)
                    .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?,
            ),
        };
        let read_u64 = |at: usize| {
            let mut value = [0_u8; 8];
            value.copy_from_slice(&input[at..at + 8]);
            u64::from_le_bytes(value)
        };
        let value = Self {
            phase,
            disposition,
            terminal: SourceFailureTerminalV1::decode(&input[16..688])?,
            product_release_binding_id: ContentId::from_bytes(read_32(688)),
            product_release_facts_id: ContentId::from_bytes(read_32(720)),
            product_release_id: ContentId::from_bytes(read_32(752)),
            product_link_account: RuntimeKey::from_bytes(read_32(784)),
            product_link_authentication_before: ContentId::from_bytes(read_32(816)),
            product_link_authentication_after: ContentId::from_bytes(read_32(848)),
            product_link_semantic_before: ContentId::from_bytes(read_32(880)),
            product_link_semantic_after: ContentId::from_bytes(read_32(912)),
            product_session_transcript_before: ContentId::from_bytes(read_32(944)),
            product_session_transcript_after: ContentId::from_bytes(read_32(976)),
            product_session_terminal_receipt_id: ContentId::from_bytes(read_32(1008)),
            product_archive_postwrite_id: ContentId::from_bytes(read_32(1040)),
            product_append_receipt_id: ContentId::from_bytes(read_32(1072)),
            product_reset_receipt_id: ContentId::from_bytes(read_32(1104)),
            product_release_preauthorization_id: ContentId::from_bytes(read_32(1136)),
            product_transition_sequence_before: read_u64(1168),
            product_transition_sequence_after: read_u64(1176),
        };
        value
            .validate_shape()
            .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?;
        Ok(value)
    }
}

/// Exact privilege/phase accepted by the current terminal account parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFailureTerminalAccountAccessV2 {
    /// Pending body created earlier in the current atomic action.
    CreatedPendingMutable,
    /// Bound post-release body written later in the current atomic action.
    BoundMutable,
    /// Existing bound evidence consumed by later Link retirement.
    ExistingBoundReadOnly,
}

/// Hostile owner/PDA/body authentication of the current terminal account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceFailureTerminalAccountV2 {
    account: RuntimeKey,
    account_data_id: ContentId,
    value: SourceFailureTerminalAccountV2,
    authentication_id: ContentId,
}

impl AuthenticatedSourceFailureTerminalAccountV2 {
    /// Physical per-occurrence Source terminal account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of the complete current account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact hostile-decoded pending or bound value.
    pub const fn value(self) -> SourceFailureTerminalAccountV2 {
        self.value
    }

    /// Complete route/PDA/body authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Authenticate one exact pending or bound current Source failure terminal.
pub fn authenticate_source_failure_terminal_account_v2(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    access: SourceFailureTerminalAccountAccessV2,
) -> Result<AuthenticatedSourceFailureTerminalAccountV2> {
    let (expected_phase, writable) = match access {
        SourceFailureTerminalAccountAccessV2::CreatedPendingMutable => (
            SourceFailureTerminalProductReleasePhaseV2::PendingProductRelease,
            true,
        ),
        SourceFailureTerminalAccountAccessV2::BoundMutable => (
            SourceFailureTerminalProductReleasePhaseV2::BoundProductRelease,
            true,
        ),
        SourceFailureTerminalAccountAccessV2::ExistingBoundReadOnly => (
            SourceFailureTerminalProductReleasePhaseV2::BoundProductRelease,
            false,
        ),
    };
    if account.owner != route.adapter_program()
        || account.executable
        || account.signer
        || account.writable != writable
    {
        return Err(Error::WrongPrivilege);
    }
    let value = SourceFailureTerminalAccountV2::decode(account.data).map_err(Error::Core)?;
    let terminal = value.terminal();
    if value.phase() != expected_phase
        || terminal.source_release_manifest_id != route.release_manifest_id()
        || terminal.source_release_authentication_id != route.release_authentication_id()
        || terminal.route_id != route.route_id()
        || terminal.source_plane_contract_id != route.source_plane_contract_id()
        || terminal.source_spec_id != route.source_spec_id()
        || terminal.source_work_schedule_id != route.source_work_schedule_id()
    {
        return Err(Error::MismatchedBinding);
    }
    let terminal_id = terminal.id()?;
    let recipe = PdaRecipeV3::source_no_reopen_terminal(terminal_id)?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        derived_pda.bump,
    )?;
    let account_data_id = account_data_id(account.key, account.data)?;
    let value_id = value.id()?;
    let mut bytes = [0_u8; 161];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&terminal_id.bytes());
    bytes[128..160].copy_from_slice(&value_id.bytes());
    bytes[160] = value.phase().wire_byte();
    Ok(AuthenticatedSourceFailureTerminalAccountV2 {
        account: account.key,
        account_data_id,
        value,
        authentication_id: domain_id(SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_AUTH_DOMAIN, &bytes),
    })
}

const fn failure_kind_byte(kind: SourceFailureKindV1) -> u8 {
    match kind {
        SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution => 1,
        SourceFailureKindV1::SourceEvaluationRefused => 2,
    }
}

fn decode_failure_kind(value: u8) -> Result<SourceFailureKindV1> {
    match value {
        1 => Ok(SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution),
        2 => Ok(SourceFailureKindV1::SourceEvaluationRefused),
        _ => Err(Error::InvalidCodec),
    }
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn key(byte: u8) -> RuntimeKey {
        RuntimeKey::from_bytes([byte; 32])
    }

    fn absence_terminal() -> SourceFailureTerminalV1 {
        let value = SourceFailureTerminalV1 {
            source_release_manifest_id: id(1),
            source_release_authentication_id: id(2),
            route_id: id(3),
            source_plane_contract_id: id(4),
            source_spec_id: id(5),
            source_work_schedule_id: id(6),
            source_reconstruction_id: id(7),
            source_handoff_id: id(8),
            source_handoff_join_id: id(9),
            persisted_handoff_authentication_id: id(10),
            source_failure_terminal_authority_id: id(11),
            market_instance_id: id(12),
            failure_policy_binding_id: id(13),
            source_fact_authentication_id: id(14),
            statistic_key_id: id(15),
            lineage_authentication_id: id(16),
            expected_lineage_state_id: id(17),
            lineage_recipe_id: id(18),
            lineage_account: key(19),
            result_or_absence_account: key(20),
            failure_generation: 1,
            source_repair_generation: 0,
            source_failure_kind:
                SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution,
            disposition: SourceFailureTerminalDispositionV1::AbsenceLineageTombstone,
        };
        value.validate_shape().expect("valid absence terminal");
        value
    }

    fn bind_absence(
        pending: SourceFailureTerminalAccountV2,
    ) -> Result<SourceFailureTerminalAccountV2> {
        pending.bind_product_release(
            SourceFailureProductReleaseDispositionV2::SourceAbsent,
            id(31),
            id(32),
            id(33),
            key(34),
            id(35),
            id(36),
            id(37),
            id(38),
            9,
            10,
            id(39),
            id(40),
            id(41),
            id(42),
            id(43),
            id(44),
            id(45),
        )
    }

    #[test]
    fn disposition_and_failure_kind_pairs_are_exhaustive() {
        for (kind, disposition, valid) in [
            (
                SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution,
                SourceFailureTerminalDispositionV1::AbsenceLineageTombstone,
                true,
            ),
            (
                SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution,
                SourceFailureTerminalDispositionV1::RefusedResultClose,
                false,
            ),
            (
                SourceFailureKindV1::SourceEvaluationRefused,
                SourceFailureTerminalDispositionV1::AbsenceLineageTombstone,
                false,
            ),
            (
                SourceFailureKindV1::SourceEvaluationRefused,
                SourceFailureTerminalDispositionV1::RefusedResultClose,
                true,
            ),
        ] {
            let pair_is_valid = matches!(
                (kind, disposition),
                (
                    SourceFailureKindV1::PrimaryMaturityWithoutAcceptedResolution,
                    SourceFailureTerminalDispositionV1::AbsenceLineageTombstone
                ) | (
                    SourceFailureKindV1::SourceEvaluationRefused,
                    SourceFailureTerminalDispositionV1::RefusedResultClose
                )
            );
            assert_eq!(pair_is_valid, valid);
        }
    }

    #[test]
    fn pending_to_bound_roundtrip_preserves_inner_terminal() {
        let terminal = absence_terminal();
        let pending = SourceFailureTerminalAccountV2::new_pending(terminal)
            .expect("valid pending wrapper");
        let mut pending_bytes = [0_u8; SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_BYTES];
        pending
            .encode_into(&mut pending_bytes)
            .expect("encode pending");
        assert_eq!(
            SourceFailureTerminalAccountV2::decode(&pending_bytes).expect("decode pending"),
            pending
        );
        let bound = bind_absence(pending).expect("one exact bind");
        assert_eq!(bound.terminal(), terminal);
        let mut bound_bytes = [0_u8; SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_BYTES];
        bound.encode_into(&mut bound_bytes).expect("encode bound");
        assert_eq!(
            SourceFailureTerminalAccountV2::decode(&bound_bytes).expect("decode bound"),
            bound
        );
    }

    #[test]
    fn duplicate_or_wrong_disposition_binding_refuses() {
        let pending = SourceFailureTerminalAccountV2::new_pending(absence_terminal())
            .expect("valid pending wrapper");
        assert!(pending
            .bind_product_release(
                SourceFailureProductReleaseDispositionV2::SourceRefused,
                id(31), id(32), id(33), key(34), id(35), id(36), id(37), id(38),
                9, 10, id(39), id(40), id(41), id(42), id(43), id(44), id(45),
            )
            .is_err());
        let bound = bind_absence(pending).expect("one exact bind");
        assert!(bind_absence(bound).is_err());
    }

    #[test]
    fn noncanonical_or_reordered_transition_evidence_refuses() {
        let pending = SourceFailureTerminalAccountV2::new_pending(absence_terminal())
            .expect("valid pending wrapper");
        assert!(pending
            .bind_product_release(
                SourceFailureProductReleaseDispositionV2::SourceAbsent,
                id(31), id(32), id(33), key(34), id(35), id(36), id(37), id(38),
                10, 9, id(39), id(40), id(41), id(42), id(43), id(44), id(45),
            )
            .is_err());
        let pending = SourceFailureTerminalAccountV2::new_pending(absence_terminal())
            .expect("valid pending wrapper");
        let mut bytes = [0_u8; SOURCE_FAILURE_TERMINAL_ACCOUNT_V2_BYTES];
        pending.encode_into(&mut bytes).expect("encode pending");
        bytes[10] = 1;
        assert!(SourceFailureTerminalAccountV2::decode(&bytes).is_err());
    }
}
