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
const SOURCE_FAILURE_TERMINAL_AUTH_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-source-failure-terminal/v1";

/// Exact fixed width of one Source failure-terminal record.
pub const SOURCE_FAILURE_TERMINAL_BYTES: usize = 672;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceFailureTerminalAccessV1 {
    CreatedMutable,
    ExistingReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceFailureTerminalV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    body: SourceFailureTerminalV1,
    authentication_id: ContentId,
}

impl AuthenticatedSourceFailureTerminalV1 {
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    pub const fn body(self) -> SourceFailureTerminalV1 {
        self.body
    }

    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

pub fn authenticate_source_failure_terminal(
    route: AuthenticatedSourceRouteV1,
    expected: SourceFailureTerminalV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    access: SourceFailureTerminalAccessV1,
) -> Result<AuthenticatedSourceFailureTerminalV1> {
    let writable = access == SourceFailureTerminalAccessV1::CreatedMutable;
    if account.owner != route.adapter_program()
        || account.executable
        || account.signer
        || account.writable != writable
    {
        return Err(Error::WrongPrivilege);
    }
    let body = SourceFailureTerminalV1::decode(account.data).map_err(Error::Core)?;
    if body != expected
        || body.source_release_manifest_id != route.release_manifest_id()
        || body.source_release_authentication_id != route.release_authentication_id()
        || body.route_id != route.route_id()
        || body.source_plane_contract_id != route.source_plane_contract_id()
        || body.source_spec_id != route.source_spec_id()
        || body.source_work_schedule_id != route.source_work_schedule_id()
    {
        return Err(Error::MismatchedBinding);
    }
    let terminal_id = body.id()?;
    let recipe = PdaRecipeV3::source_no_reopen_terminal(terminal_id)?;
    derived_pda.validate_for(
        route.adapter_program(),
        recipe.id()?,
        account.key,
        derived_pda.bump,
    )?;
    let account_data_id = account_data_id(account.key, account.data)?;
    let mut bytes = [0_u8; 160];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&terminal_id.bytes());
    bytes[128..160].copy_from_slice(&body.lineage_authentication_id.bytes());
    Ok(AuthenticatedSourceFailureTerminalV1 {
        account: account.key,
        account_data_id,
        body,
        authentication_id: domain_id(SOURCE_FAILURE_TERMINAL_AUTH_DOMAIN, &bytes),
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
}
