use clutch_source_plane_v3::{ContentId, FixedCodec};
use clutch_source_plane_v3_adapter::PdaRecipeV3;

use crate::auth::{
    account_data_id, domain_id, AuthenticatedSourceRouteV1, RuntimeAccountViewV1,
    RuntimeDerivedPdaV1, RuntimeKey,
};
use crate::lineage::{AuthenticatedReopenLineageV1, LineageAccessV1, LineageFamilyV1};
use crate::reopen::SourceReopenFamilyV1;
use crate::{Error, Result};

const NO_REOPEN_TERMINAL_MAGIC: [u8; 8] = *b"DCSPNRT1";
const NO_REOPEN_TERMINAL_DOMAIN: &[u8] =
    b"dragons-clutch/source-no-reopen-terminal/v1";
const NO_REOPEN_TERMINAL_AUTH_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-source-no-reopen-terminal/v1";
const NO_REOPEN_DISPOSITION: u8 = 1;

/// Exact hostile-codec width of one immutable no-reopen terminal record.
pub const SOURCE_NO_REOPEN_TERMINAL_BYTES: usize = 608;

/// Immutable Source-owned proof that one resolved mutable family cannot be
/// reopened after the shared Product ResolutionV5 and Failure cell postwrites.
///
/// The body contains no payout, relation classification, or caller-selected
/// replacement state. A later action-12 close consumes the Source terminal
/// work receipt minted from this record's content identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceNoReopenTerminalV1 {
    source_release_manifest_id: ContentId,
    source_release_authentication_id: ContentId,
    route_id: ContentId,
    source_plane_contract_id: ContentId,
    source_spec_id: ContentId,
    source_work_schedule_id: ContentId,
    generation_authority_program: RuntimeKey,
    source_resolution_input_id: ContentId,
    successful_handoff_id: ContentId,
    failure_policy_binding_id: ContentId,
    failure_resolution_receipt_id: ContentId,
    resolution_v5_terminal_postwrite_id: ContentId,
    market_instance_id: ContentId,
    lineage_authentication_id: ContentId,
    lineage_recipe_id: ContentId,
    expected_lineage_state_id: ContentId,
    lineage_account: RuntimeKey,
    target_account: RuntimeKey,
    failure_generation: u64,
    source_repair_generation: u64,
    family: SourceReopenFamilyV1,
}

impl SourceNoReopenTerminalV1 {
    /// Construct the only terminal decision admitted after successful
    /// ResolutionV5: the exact open lineage is selected for permanent close,
    /// with no GenerationAuthority replacement body.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        route: AuthenticatedSourceRouteV1,
        source_resolution_input_id: ContentId,
        successful_handoff_id: ContentId,
        failure_policy_binding_id: ContentId,
        failure_resolution_receipt_id: ContentId,
        resolution_v5_terminal_postwrite_id: ContentId,
        market_instance_id: ContentId,
        failure_generation: u64,
        source_repair_generation: u64,
        family: SourceReopenFamilyV1,
        lineage: AuthenticatedReopenLineageV1,
    ) -> Result<Self> {
        let state = lineage.lineage();
        if lineage.access() != LineageAccessV1::Mutable
            || !state.is_open
            || state.family != lineage_family(family)
        {
            return Err(Error::InvalidLineage);
        }
        let value = Self {
            source_release_manifest_id: route.release_manifest_id(),
            source_release_authentication_id: route.release_authentication_id(),
            route_id: route.route_id(),
            source_plane_contract_id: route.source_plane_contract_id(),
            source_spec_id: route.source_spec_id(),
            source_work_schedule_id: route.source_work_schedule_id(),
            generation_authority_program: route.generation_authority_program(),
            source_resolution_input_id,
            successful_handoff_id,
            failure_policy_binding_id,
            failure_resolution_receipt_id,
            resolution_v5_terminal_postwrite_id,
            market_instance_id,
            lineage_authentication_id: lineage.id(),
            lineage_recipe_id: state.recipe_id()?,
            expected_lineage_state_id: lineage.account_data_id(),
            lineage_account: state.lineage_account,
            target_account: state.active_account,
            failure_generation,
            source_repair_generation,
            family,
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
            self.source_resolution_input_id,
            self.successful_handoff_id,
            self.failure_policy_binding_id,
            self.failure_resolution_receipt_id,
            self.resolution_v5_terminal_postwrite_id,
            self.market_instance_id,
            self.lineage_authentication_id,
            self.lineage_recipe_id,
            self.expected_lineage_state_id,
        ] {
            if id.is_zero() {
                return Err(Error::ZeroIdentity);
            }
        }
        self.generation_authority_program.validate()?;
        self.lineage_account.validate()?;
        self.target_account.validate()?;
        if self.failure_generation == 0
            || self.generation_authority_program == self.lineage_account
            || self.generation_authority_program == self.target_account
            || self.lineage_account == self.target_account
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Complete immutable body identity and PDA coordinate.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0_u8; SOURCE_NO_REOPEN_TERMINAL_BYTES];
        self.encode_into(&mut bytes).map_err(Error::Core)?;
        Ok(domain_id(NO_REOPEN_TERMINAL_DOMAIN, &bytes))
    }

    /// Selected mutable Source family which may now only close.
    pub const fn family(self) -> SourceReopenFamilyV1 {
        self.family
    }

    /// Exact open-lineage account preimage authenticated before terminal mint.
    pub const fn expected_lineage_state_id(self) -> ContentId {
        self.expected_lineage_state_id
    }

    /// Exact lineage authentication retained when the policy was selected.
    pub const fn lineage_authentication_id(self) -> ContentId {
        self.lineage_authentication_id
    }

    /// Durable lineage account selected by the terminal policy.
    pub const fn lineage_account(self) -> RuntimeKey {
        self.lineage_account
    }

    /// Exact active generation account selected for permanent close.
    pub const fn target_account(self) -> RuntimeKey {
        self.target_account
    }

    /// Exact private ResolutionV5/Failure postwrite consumed by this decision.
    pub const fn resolution_v5_terminal_postwrite_id(self) -> ContentId {
        self.resolution_v5_terminal_postwrite_id
    }
}

impl FixedCodec for SourceNoReopenTerminalV1 {
    const ENCODED_LEN: usize = SOURCE_NO_REOPEN_TERMINAL_BYTES;

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
        output[..8].copy_from_slice(&NO_REOPEN_TERMINAL_MAGIC);
        output[8..10].copy_from_slice(&1_u16.to_le_bytes());
        output[10] = self.family.wire_byte();
        output[11] = NO_REOPEN_DISPOSITION;
        let values = [
            self.source_release_manifest_id.bytes(),
            self.source_release_authentication_id.bytes(),
            self.route_id.bytes(),
            self.source_plane_contract_id.bytes(),
            self.source_spec_id.bytes(),
            self.source_work_schedule_id.bytes(),
            self.generation_authority_program.bytes(),
            self.source_resolution_input_id.bytes(),
            self.successful_handoff_id.bytes(),
            self.failure_policy_binding_id.bytes(),
            self.failure_resolution_receipt_id.bytes(),
            self.resolution_v5_terminal_postwrite_id.bytes(),
            self.market_instance_id.bytes(),
            self.lineage_authentication_id.bytes(),
            self.lineage_recipe_id.bytes(),
            self.expected_lineage_state_id.bytes(),
            self.lineage_account.bytes(),
            self.target_account.bytes(),
        ];
        let mut at = 16_usize;
        for value in values {
            output[at..at + 32].copy_from_slice(&value);
            at += 32;
        }
        output[592..600].copy_from_slice(&self.failure_generation.to_le_bytes());
        output[600..608].copy_from_slice(&self.source_repair_generation.to_le_bytes());
        Ok(())
    }

    fn decode(input: &[u8]) -> core::result::Result<Self, clutch_source_plane_v3::Error> {
        if input.len() < Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::Truncated);
        }
        if input.len() > Self::ENCODED_LEN {
            return Err(clutch_source_plane_v3::Error::TrailingBytes);
        }
        if input[..8] != NO_REOPEN_TERMINAL_MAGIC {
            return Err(clutch_source_plane_v3::Error::BadMagic);
        }
        if input[8..10] != 1_u16.to_le_bytes() {
            return Err(clutch_source_plane_v3::Error::BadVersion);
        }
        if input[11] != NO_REOPEN_DISPOSITION
            || input[12..16].iter().any(|byte| *byte != 0)
        {
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
        let value = Self {
            source_release_manifest_id: ContentId::from_bytes(read_32(16)),
            source_release_authentication_id: ContentId::from_bytes(read_32(48)),
            route_id: ContentId::from_bytes(read_32(80)),
            source_plane_contract_id: ContentId::from_bytes(read_32(112)),
            source_spec_id: ContentId::from_bytes(read_32(144)),
            source_work_schedule_id: ContentId::from_bytes(read_32(176)),
            generation_authority_program: RuntimeKey::from_bytes(read_32(208)),
            source_resolution_input_id: ContentId::from_bytes(read_32(240)),
            successful_handoff_id: ContentId::from_bytes(read_32(272)),
            failure_policy_binding_id: ContentId::from_bytes(read_32(304)),
            failure_resolution_receipt_id: ContentId::from_bytes(read_32(336)),
            resolution_v5_terminal_postwrite_id: ContentId::from_bytes(read_32(368)),
            market_instance_id: ContentId::from_bytes(read_32(400)),
            lineage_authentication_id: ContentId::from_bytes(read_32(432)),
            lineage_recipe_id: ContentId::from_bytes(read_32(464)),
            expected_lineage_state_id: ContentId::from_bytes(read_32(496)),
            lineage_account: RuntimeKey::from_bytes(read_32(528)),
            target_account: RuntimeKey::from_bytes(read_32(560)),
            failure_generation: read_u64(592),
            source_repair_generation: read_u64(600),
            family: decode_family(input[10])
                .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?,
        };
        value
            .validate_shape()
            .map_err(|_| clutch_source_plane_v3::Error::MismatchedArtifact)?;
        Ok(value)
    }
}

/// Privilege mode for the immutable no-reopen terminal account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceNoReopenTerminalAccessV1 {
    /// Same-instruction postwrite authentication.
    CreatedMutable,
    /// Later read-only audit of the permanent terminal decision.
    ExistingReadOnly,
}

/// Private owner/PDA/body receipt for one explicit no-reopen terminal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceNoReopenTerminalV1 {
    account: RuntimeKey,
    account_data_id: ContentId,
    body: SourceNoReopenTerminalV1,
    authentication_id: ContentId,
}

impl AuthenticatedSourceNoReopenTerminalV1 {
    /// Physical immutable Source terminal account.
    pub const fn account(self) -> RuntimeKey {
        self.account
    }

    /// Digest of complete persisted bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Exact no-reopen semantic identity.
    pub fn terminal_id(self) -> Result<ContentId> {
        self.body.id()
    }

    /// Exact final Product/Failure ResolutionV5 postwrite retained by this
    /// no-reopen decision.
    pub const fn resolution_v5_terminal_postwrite_id(self) -> ContentId {
        self.body.resolution_v5_terminal_postwrite_id()
    }

    /// Exact private Source resolution input selected by this terminal.
    pub const fn source_resolution_input_id(self) -> ContentId {
        self.body.source_resolution_input_id
    }

    /// Mutable family selected for permanent close.
    pub const fn family(self) -> SourceReopenFamilyV1 {
        self.body.family()
    }

    /// Exact open-lineage account preimage authenticated before terminal mint.
    pub const fn expected_lineage_state_id(self) -> ContentId {
        self.body.expected_lineage_state_id()
    }

    /// Exact lineage authentication retained before terminal mint.
    pub const fn lineage_authentication_id(self) -> ContentId {
        self.body.lineage_authentication_id()
    }

    /// Durable lineage account selected for close.
    pub const fn lineage_account(self) -> RuntimeKey {
        self.body.lineage_account()
    }

    /// Exact active generation account selected for close.
    pub const fn target_account(self) -> RuntimeKey {
        self.body.target_account()
    }

    /// Complete owner/PDA/body authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Authenticate one exact Source-owned no-reopen terminal account against
/// the private body reconstructed by the ResolutionV5 terminal composer.
pub fn authenticate_source_no_reopen_terminal(
    route: AuthenticatedSourceRouteV1,
    expected: SourceNoReopenTerminalV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    access: SourceNoReopenTerminalAccessV1,
) -> Result<AuthenticatedSourceNoReopenTerminalV1> {
    if account.owner != route.adapter_program() {
        return Err(Error::WrongOwner);
    }
    if account.executable
        || account.signer
        || account.writable != (access == SourceNoReopenTerminalAccessV1::CreatedMutable)
    {
        return Err(Error::WrongPrivilege);
    }
    let body = SourceNoReopenTerminalV1::decode(account.data).map_err(Error::Core)?;
    if body != expected
        || body.source_release_manifest_id != route.release_manifest_id()
        || body.source_release_authentication_id != route.release_authentication_id()
        || body.route_id != route.route_id()
        || body.source_plane_contract_id != route.source_plane_contract_id()
        || body.source_spec_id != route.source_spec_id()
        || body.source_work_schedule_id != route.source_work_schedule_id()
        || body.generation_authority_program != route.generation_authority_program()
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
    Ok(AuthenticatedSourceNoReopenTerminalV1 {
        account: account.key,
        account_data_id,
        body,
        authentication_id: domain_id(NO_REOPEN_TERMINAL_AUTH_DOMAIN, &bytes),
    })
}

fn lineage_family(family: SourceReopenFamilyV1) -> LineageFamilyV1 {
    match family {
        SourceReopenFamilyV1::SourceHead => LineageFamilyV1::SourceHead,
        SourceReopenFamilyV1::OpenRawPage => LineageFamilyV1::OpenRawPage,
        SourceReopenFamilyV1::WindowWork => LineageFamilyV1::WindowWork,
        SourceReopenFamilyV1::StatisticResult => LineageFamilyV1::StatisticResult,
    }
}

fn decode_family(value: u8) -> Result<SourceReopenFamilyV1> {
    match value {
        1 => Ok(SourceReopenFamilyV1::SourceHead),
        2 => Ok(SourceReopenFamilyV1::OpenRawPage),
        3 => Ok(SourceReopenFamilyV1::WindowWork),
        4 => Ok(SourceReopenFamilyV1::StatisticResult),
        _ => Err(Error::InvalidCodec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn key(byte: u8) -> RuntimeKey {
        RuntimeKey::from_bytes([byte; 32])
    }

    fn record() -> SourceNoReopenTerminalV1 {
        SourceNoReopenTerminalV1 {
            source_release_manifest_id: id(1),
            source_release_authentication_id: id(2),
            route_id: id(3),
            source_plane_contract_id: id(4),
            source_spec_id: id(5),
            source_work_schedule_id: id(6),
            generation_authority_program: key(7),
            source_resolution_input_id: id(8),
            successful_handoff_id: id(9),
            failure_policy_binding_id: id(10),
            failure_resolution_receipt_id: id(11),
            resolution_v5_terminal_postwrite_id: id(12),
            market_instance_id: id(13),
            lineage_authentication_id: id(14),
            lineage_recipe_id: id(15),
            expected_lineage_state_id: id(16),
            lineage_account: key(17),
            target_account: key(18),
            failure_generation: 19,
            source_repair_generation: 20,
            family: SourceReopenFamilyV1::WindowWork,
        }
    }

    #[test]
    fn no_reopen_terminal_codec_refuses_spliced_shape() {
        let value = record();
        let mut bytes = [0_u8; SOURCE_NO_REOPEN_TERMINAL_BYTES];
        value.encode_into(&mut bytes).unwrap();
        assert_eq!(SourceNoReopenTerminalV1::decode(&bytes), Ok(value));

        let mut hostile = bytes;
        hostile[12] = 1;
        assert_eq!(
            SourceNoReopenTerminalV1::decode(&hostile),
            Err(clutch_source_plane_v3::Error::NonCanonicalReserved)
        );
        let mut hostile = bytes;
        hostile[11] = 2;
        assert_eq!(
            SourceNoReopenTerminalV1::decode(&hostile),
            Err(clutch_source_plane_v3::Error::NonCanonicalReserved)
        );
        let mut hostile = bytes;
        hostile[10] = 5;
        assert_eq!(
            SourceNoReopenTerminalV1::decode(&hostile),
            Err(clutch_source_plane_v3::Error::MismatchedArtifact)
        );
        let mut hostile = bytes;
        hostile[240..272].fill(0);
        assert_eq!(
            SourceNoReopenTerminalV1::decode(&hostile),
            Err(clutch_source_plane_v3::Error::MismatchedArtifact)
        );
        assert_eq!(
            SourceNoReopenTerminalV1::decode(&bytes[..bytes.len() - 1]),
            Err(clutch_source_plane_v3::Error::Truncated)
        );
    }
}
