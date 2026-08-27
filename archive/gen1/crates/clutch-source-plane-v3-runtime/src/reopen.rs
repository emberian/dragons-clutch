use clutch_source_plane_v3::{
    ContentId, FixedCodec, OpenRawPageV3, SourceHeadV3, StatisticResultV3, WindowWorkV3,
    OPEN_RAW_PAGE_BYTES, SOURCE_HEAD_BYTES, STATISTIC_RESULT_BYTES, WINDOW_WORK_BYTES,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;

use crate::auth::{
    account_data_id, domain_id, AuthenticatedSourceRouteV1, RuntimeAccountViewV1,
    RuntimeDerivedPdaV1, RuntimeKey,
};
use crate::lineage::{
    close_lineage_generation, AuthenticatedReopenLineageV1, LineageAccessV1,
    LineageFamilyV1,
};
use crate::{Error, Result};

const REOPEN_REQUEST_MAGIC: [u8; 8] = *b"DCSRPN01";
const REOPEN_REQUEST_DOMAIN: &[u8] = b"dragons-clutch/source-reopen-request/v1";
const REOPEN_TARGET_DOMAIN: &[u8] = b"dragons-clutch/source-reopen-target-body/v1";
const REOPEN_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-source-reopen/v1";
const REOPEN_PRECLOSE_AUTH_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-source-reopen-preclose/v1";

/// Maximum exact semantic body carried by a release-selected reopen request.
pub const SOURCE_REOPEN_TARGET_BODY_BYTES: usize = OPEN_RAW_PAGE_BYTES;
/// Exact hostile codec width of one persisted reopen request.
pub const SOURCE_REOPEN_GENERATION_REQUEST_BYTES: usize =
    256 + SOURCE_REOPEN_TARGET_BODY_BYTES;

/// Closed mutable family selected by an immutable Product/Failure reopen request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SourceReopenFamilyV1 {
    /// Source-only generation head.
    SourceHead = 1,
    /// Mutable page-ingestion prefix.
    OpenRawPage = 2,
    /// Resumable Window fold.
    WindowWork = 3,
    /// Persisted evaluator result.
    StatisticResult = 4,
}

impl SourceReopenFamilyV1 {
    /// Canonical wire discriminator without an unchecked enum cast.
    pub const fn wire_byte(self) -> u8 {
        match self {
            Self::SourceHead => 1,
            Self::OpenRawPage => 2,
            Self::WindowWork => 3,
            Self::StatisticResult => 4,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::SourceHead),
            2 => Ok(Self::OpenRawPage),
            3 => Ok(Self::WindowWork),
            4 => Ok(Self::StatisticResult),
            _ => Err(Error::InvalidCodec),
        }
    }

    const fn lineage_family(self) -> LineageFamilyV1 {
        match self {
            Self::SourceHead => LineageFamilyV1::SourceHead,
            Self::OpenRawPage => LineageFamilyV1::OpenRawPage,
            Self::WindowWork => LineageFamilyV1::WindowWork,
            Self::StatisticResult => LineageFamilyV1::StatisticResult,
        }
    }
}

/// Exact typed semantic postimage persisted by the selected generation owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceReopenTargetV1 {
    /// Complete canonical SourceHead body.
    SourceHead(SourceHeadV3),
    /// Complete canonical OpenRawPage body.
    OpenRawPage(OpenRawPageV3),
    /// Complete canonical WindowWork body.
    WindowWork(WindowWorkV3),
    /// Complete canonical StatisticResult body.
    StatisticResult(StatisticResultV3),
}

impl SourceReopenTargetV1 {
    /// Closed wire/runtime family of this postimage.
    pub const fn family(&self) -> SourceReopenFamilyV1 {
        match self {
            Self::SourceHead(_) => SourceReopenFamilyV1::SourceHead,
            Self::OpenRawPage(_) => SourceReopenFamilyV1::OpenRawPage,
            Self::WindowWork(_) => SourceReopenFamilyV1::WindowWork,
            Self::StatisticResult(_) => SourceReopenFamilyV1::StatisticResult,
        }
    }

    const fn encoded_len(&self) -> usize {
        match self {
            Self::SourceHead(_) => SOURCE_HEAD_BYTES,
            Self::OpenRawPage(_) => OPEN_RAW_PAGE_BYTES,
            Self::WindowWork(_) => WINDOW_WORK_BYTES,
            Self::StatisticResult(_) => STATISTIC_RESULT_BYTES,
        }
    }

    fn encode_body(&self, output: &mut [u8]) -> Result<usize> {
        if output.len() != SOURCE_REOPEN_TARGET_BODY_BYTES {
            return Err(Error::InvalidCodec);
        }
        output.fill(0);
        let len = self.encoded_len();
        match self {
            Self::SourceHead(value) => value.encode_into(&mut output[..len]),
            Self::OpenRawPage(value) => value.encode_into(&mut output[..len]),
            Self::WindowWork(value) => value.encode_into(&mut output[..len]),
            Self::StatisticResult(value) => value.encode_into(&mut output[..len]),
        }
        .map_err(Error::Core)?;
        Ok(len)
    }

    fn decode_body(
        family: SourceReopenFamilyV1,
        body_len: usize,
        input: &[u8],
    ) -> Result<Self> {
        let expected = match family {
            SourceReopenFamilyV1::SourceHead => SOURCE_HEAD_BYTES,
            SourceReopenFamilyV1::OpenRawPage => OPEN_RAW_PAGE_BYTES,
            SourceReopenFamilyV1::WindowWork => WINDOW_WORK_BYTES,
            SourceReopenFamilyV1::StatisticResult => STATISTIC_RESULT_BYTES,
        };
        if input.len() != SOURCE_REOPEN_TARGET_BODY_BYTES
            || body_len != expected
            || input[body_len..].iter().any(|byte| *byte != 0)
        {
            return Err(Error::InvalidCodec);
        }
        match family {
            SourceReopenFamilyV1::SourceHead => Ok(Self::SourceHead(
                SourceHeadV3::decode(&input[..body_len]).map_err(Error::Core)?,
            )),
            SourceReopenFamilyV1::OpenRawPage => Ok(Self::OpenRawPage(
                OpenRawPageV3::decode(&input[..body_len]).map_err(Error::Core)?,
            )),
            SourceReopenFamilyV1::WindowWork => Ok(Self::WindowWork(
                WindowWorkV3::decode(&input[..body_len]).map_err(Error::Core)?,
            )),
            SourceReopenFamilyV1::StatisticResult => Ok(Self::StatisticResult(
                StatisticResultV3::decode(&input[..body_len]).map_err(Error::Core)?,
            )),
        }
    }

    /// Digest of the family discriminator and complete canonical core body.
    pub fn body_id(&self) -> Result<ContentId> {
        let mut bytes = [0_u8; 8 + SOURCE_REOPEN_TARGET_BODY_BYTES];
        let len = self.encode_body(&mut bytes[8..])?;
        bytes[0] = self.family().wire_byte();
        bytes[2..4].copy_from_slice(
            &u16::try_from(len)
                .map_err(|_| Error::ArithmeticOverflow)?
                .to_le_bytes(),
        );
        Ok(domain_id(REOPEN_TARGET_DOMAIN, &bytes))
    }

    /// Derive the sole physical target recipe and semantic lineage coordinate.
    pub fn recipe(&self, route: AuthenticatedSourceRouteV1) -> Result<PdaRecipeV3> {
        match self {
            Self::SourceHead(value) => {
                if value.source_spec_id != route.source_spec_id() {
                    return Err(Error::MismatchedBinding);
                }
                PdaRecipeV3::source_head(
                    route.source_plane_contract_id(),
                    route.source_spec_id(),
                    value.repair_generation,
                )
                .map_err(Error::Adapter)
            }
            Self::OpenRawPage(value) => {
                if value.source_spec_id != route.source_spec_id() {
                    return Err(Error::MismatchedBinding);
                }
                PdaRecipeV3::open_raw_page(
                    route.source_plane_contract_id(),
                    route.source_spec_id(),
                    value.repair_generation,
                    value.page_index,
                )
                .map_err(Error::Adapter)
            }
            Self::WindowWork(value) => {
                PdaRecipeV3::window_work(value.window_id()).map_err(Error::Adapter)
            }
            Self::StatisticResult(value) => {
                PdaRecipeV3::statistic_result(value.statistic_key_id()).map_err(Error::Adapter)
            }
        }
    }
}

/// Immutable current-owner authorization for one exact closed-lineage reopen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceReopenGenerationRequestV1 {
    /// Current Source release manifest.
    pub source_release_manifest_id: ContentId,
    /// Complete authenticated route identity.
    pub route_id: ContentId,
    /// Exact reviewed SourcePlane semantic contract.
    pub source_plane_contract_id: ContentId,
    /// Release-selected SourceSpec.
    pub source_spec_id: ContentId,
    /// Release-selected heterogeneous work schedule.
    pub source_work_schedule_id: ContentId,
    /// Digest of the exact closed lineage account preimage.
    pub expected_lineage_state_id: ContentId,
    /// Immutable Product/Failure repair-policy request.
    pub generation_policy_id: ContentId,
    /// Complete typed target postimage; no instruction payload supplies it.
    pub target: SourceReopenTargetV1,
}

impl SourceReopenGenerationRequestV1 {
    /// Reconstruct the sole request body for an already projected closed
    /// lineage. No instruction payload supplies target bytes or lineage facts.
    pub fn new(
        route: AuthenticatedSourceRouteV1,
        expected_lineage_state_id: ContentId,
        generation_policy_id: ContentId,
        target: SourceReopenTargetV1,
        closed_lineage: crate::lineage::ReopenLineageV1,
    ) -> Result<Self> {
        let value = Self {
            source_release_manifest_id: route.release_manifest_id(),
            route_id: route.route_id(),
            source_plane_contract_id: route.source_plane_contract_id(),
            source_spec_id: route.source_spec_id(),
            source_work_schedule_id: route.source_work_schedule_id(),
            expected_lineage_state_id,
            generation_policy_id,
            target,
        };
        value.validate_against_state(route, closed_lineage, expected_lineage_state_id)?;
        Ok(value)
    }

    fn validate_shape(&self) -> Result<()> {
        for id in [
            self.source_release_manifest_id,
            self.route_id,
            self.source_plane_contract_id,
            self.source_spec_id,
            self.source_work_schedule_id,
            self.expected_lineage_state_id,
            self.generation_policy_id,
            self.target.body_id()?,
        ] {
            if id.is_zero() {
                return Err(Error::ZeroIdentity);
            }
        }
        Ok(())
    }

    /// Encode the exact fixed-width hostile account body.
    pub fn encode(&self) -> Result<[u8; SOURCE_REOPEN_GENERATION_REQUEST_BYTES]> {
        self.validate_shape()?;
        let mut out = [0_u8; SOURCE_REOPEN_GENERATION_REQUEST_BYTES];
        out[..8].copy_from_slice(&REOPEN_REQUEST_MAGIC);
        out[8..10].copy_from_slice(&1_u16.to_le_bytes());
        out[10] = self.target.family().wire_byte();
        let ids = [
            self.source_release_manifest_id,
            self.route_id,
            self.source_plane_contract_id,
            self.source_spec_id,
            self.source_work_schedule_id,
            self.expected_lineage_state_id,
            self.generation_policy_id,
        ];
        let mut at = 16_usize;
        for id in ids {
            out[at..at + 32].copy_from_slice(&id.bytes());
            at += 32;
        }
        let body_len = self.target.encode_body(&mut out[256..])?;
        out[240..242].copy_from_slice(
            &u16::try_from(body_len)
                .map_err(|_| Error::ArithmeticOverflow)?
                .to_le_bytes(),
        );
        Ok(out)
    }

    /// Hostile-decode one exact persisted owner request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != SOURCE_REOPEN_GENERATION_REQUEST_BYTES
            || input[..8] != REOPEN_REQUEST_MAGIC
            || input[8..10] != 1_u16.to_le_bytes()
            || input[11..16].iter().any(|byte| *byte != 0)
            || input[242..256].iter().any(|byte| *byte != 0)
        {
            return Err(Error::InvalidCodec);
        }
        let family = SourceReopenFamilyV1::decode(input[10])?;
        let read_id = |at: usize| {
            let mut bytes = [0_u8; 32];
            bytes.copy_from_slice(&input[at..at + 32]);
            ContentId::from_bytes(bytes)
        };
        let mut body_len = [0_u8; 2];
        body_len.copy_from_slice(&input[240..242]);
        let body_len = usize::from(u16::from_le_bytes(body_len));
        let value = Self {
            source_release_manifest_id: read_id(16),
            route_id: read_id(48),
            source_plane_contract_id: read_id(80),
            source_spec_id: read_id(112),
            source_work_schedule_id: read_id(144),
            expected_lineage_state_id: read_id(176),
            generation_policy_id: read_id(208),
            target: SourceReopenTargetV1::decode_body(family, body_len, &input[256..])?,
        };
        value.validate_shape()?;
        Ok(value)
    }

    /// Content identity of the complete current-owner request.
    pub fn id(&self) -> Result<ContentId> {
        Ok(domain_id(REOPEN_REQUEST_DOMAIN, &self.encode()?))
    }

    fn validate_against(
        &self,
        route: AuthenticatedSourceRouteV1,
        lineage: AuthenticatedReopenLineageV1,
    ) -> Result<()> {
        self.validate_against_state(route, lineage.lineage(), lineage.account_data_id())?;
        if lineage.access() != LineageAccessV1::Mutable {
            return Err(Error::WrongPrivilege);
        }
        Ok(())
    }

    fn validate_against_state(
        &self,
        route: AuthenticatedSourceRouteV1,
        state: crate::lineage::ReopenLineageV1,
        lineage_state_id: ContentId,
    ) -> Result<()> {
        self.validate_shape()?;
        let recipe = self.target.recipe(route)?;
        if self.source_release_manifest_id != route.release_manifest_id()
            || self.route_id != route.route_id()
            || self.source_plane_contract_id != route.source_plane_contract_id()
            || self.source_spec_id != route.source_spec_id()
            || self.source_work_schedule_id != route.source_work_schedule_id()
            || self.expected_lineage_state_id != lineage_state_id
            || state.family != self.target.family().lineage_family()
            || state.semantic_binding_id != recipe.id()?
            || state.is_open
            || state.latest_generation == 0
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Exact closed-lineage account digest expected by action 11.
    pub const fn expected_lineage_state_id(self) -> ContentId {
        self.expected_lineage_state_id
    }

    /// Exact private terminal policy which requested this generation.
    pub const fn generation_policy_id(self) -> ContentId {
        self.generation_policy_id
    }

    /// Complete typed target reconstructed by the terminal policy owner.
    pub const fn target(self) -> SourceReopenTargetV1 {
        self.target
    }
}

/// Private receipt for one exact selected-owner request and closed lineage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceReopenGenerationV1 {
    request_account: RuntimeKey,
    request: SourceReopenGenerationRequestV1,
    authorization_id: ContentId,
}

/// Private proof that one persisted request exactly predicts the action-12
/// closed-lineage postimage. It grants no reopen authority while the lineage
/// remains open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSourceReopenPrecloseV1 {
    request_account: RuntimeKey,
    request: SourceReopenGenerationRequestV1,
    projected_closed_lineage_state_id: ContentId,
    authentication_id: ContentId,
}

impl AuthenticatedSourceReopenPrecloseV1 {
    /// Physical immutable request account authenticated before close.
    pub const fn request_account(self) -> RuntimeKey {
        self.request_account
    }

    /// Exact private terminal policy carried by the projected close.
    pub const fn generation_policy_id(self) -> ContentId {
        self.request.generation_policy_id
    }

    /// Exact currently-open family which may close before this request opens.
    pub const fn family(self) -> SourceReopenFamilyV1 {
        self.request.target.family()
    }

    /// Digest of the sole closed-lineage postimage accepted by action 11.
    pub const fn projected_closed_lineage_state_id(self) -> ContentId {
        self.projected_closed_lineage_state_id
    }

    /// Complete request/PDA/open-lineage/projection authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

impl AuthenticatedSourceReopenGenerationV1 {
    /// Physical immutable current-owner request account.
    pub const fn request_account(self) -> RuntimeKey {
        self.request_account
    }

    /// Complete typed target supplied only by the authenticated owner record.
    pub const fn target(&self) -> &SourceReopenTargetV1 {
        &self.request.target
    }

    /// Expected exact closed-lineage account preimage digest.
    pub const fn expected_lineage_state_id(self) -> ContentId {
        self.request.expected_lineage_state_id
    }

    /// Exact current Product/Failure repair policy request.
    pub const fn generation_policy_id(self) -> ContentId {
        self.request.generation_policy_id
    }

    /// Complete request-account/route/lineage authentication identity.
    pub const fn id(self) -> ContentId {
        self.authorization_id
    }
}

/// Authenticate the immutable owner request and its exact closed lineage.
pub fn authenticate_source_reopen_generation_request(
    route: AuthenticatedSourceRouteV1,
    request_account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    lineage: AuthenticatedReopenLineageV1,
) -> Result<AuthenticatedSourceReopenGenerationV1> {
    if request_account.owner != route.generation_authority_program() {
        return Err(Error::WrongOwner);
    }
    if request_account.executable || request_account.signer || request_account.writable {
        return Err(Error::WrongPrivilege);
    }
    let request = SourceReopenGenerationRequestV1::decode(request_account.data)?;
    request.validate_against(route, lineage)?;
    derived_pda.validate_for(
        route.generation_authority_program(),
        request.id()?,
        request_account.key,
        derived_pda.bump,
    )?;
    let account_data_id = account_data_id(request_account.key, request_account.data)?;
    let mut bytes = [0_u8; 160];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&request_account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&request.id()?.bytes());
    bytes[128..160].copy_from_slice(&lineage.id().bytes());
    Ok(AuthenticatedSourceReopenGenerationV1 {
        request_account: request_account.key,
        request,
        authorization_id: domain_id(REOPEN_AUTH_DOMAIN, &bytes),
    })
}

/// Authenticate an immutable reopen request before action 12 closes its exact
/// currently-open lineage. The request must already commit to the deterministic
/// closed-lineage postimage and the terminal receipt semantic used by the
/// close; this receipt cannot itself authorize action 11.
pub fn authenticate_source_reopen_generation_request_before_close(
    route: AuthenticatedSourceRouteV1,
    request_account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    lineage: AuthenticatedReopenLineageV1,
    terminal_semantic_id: ContentId,
) -> Result<AuthenticatedSourceReopenPrecloseV1> {
    if request_account.owner != route.generation_authority_program() {
        return Err(Error::WrongOwner);
    }
    if request_account.executable || request_account.signer || request_account.writable {
        return Err(Error::WrongPrivilege);
    }
    if terminal_semantic_id.is_zero()
        || lineage.access() != LineageAccessV1::Mutable
        || !lineage.lineage().is_open
    {
        return Err(Error::InvalidLineage);
    }
    let request = SourceReopenGenerationRequestV1::decode(request_account.data)?;
    if request.generation_policy_id != terminal_semantic_id {
        return Err(Error::MismatchedBinding);
    }
    let state = lineage.lineage();
    let projected_closed = close_lineage_generation(
        state,
        state.active_account,
        state.latest_generation,
        state.last_opened_state_id,
        terminal_semantic_id,
    )?;
    let projected_bytes = projected_closed.encode()?;
    let projected_closed_lineage_state_id =
        account_data_id(state.lineage_account, &projected_bytes)?;
    request.validate_against_state(
        route,
        projected_closed,
        projected_closed_lineage_state_id,
    )?;
    derived_pda.validate_for(
        route.generation_authority_program(),
        request.id()?,
        request_account.key,
        derived_pda.bump,
    )?;
    let request_account_data_id = account_data_id(request_account.key, request_account.data)?;
    let mut bytes = [0_u8; 192];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&request_account.key.bytes());
    bytes[64..96].copy_from_slice(&request_account_data_id.bytes());
    bytes[96..128].copy_from_slice(&request.id()?.bytes());
    bytes[128..160].copy_from_slice(&lineage.id().bytes());
    bytes[160..192].copy_from_slice(&projected_closed_lineage_state_id.bytes());
    Ok(AuthenticatedSourceReopenPrecloseV1 {
        request_account: request_account.key,
        request,
        projected_closed_lineage_state_id,
        authentication_id: domain_id(REOPEN_PRECLOSE_AUTH_DOMAIN, &bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn request() -> SourceReopenGenerationRequestV1 {
        SourceReopenGenerationRequestV1 {
            source_release_manifest_id: id(1),
            route_id: id(2),
            source_plane_contract_id: id(3),
            source_spec_id: id(4),
            source_work_schedule_id: id(5),
            expected_lineage_state_id: id(6),
            generation_policy_id: id(7),
            target: SourceReopenTargetV1::SourceHead(SourceHeadV3::new(id(4), 8, 9).unwrap()),
        }
    }

    #[test]
    fn reopen_request_codec_refuses_family_length_and_padding_splices() {
        let request = request();
        let bytes = request.encode().unwrap();
        assert_eq!(
            SourceReopenGenerationRequestV1::decode(&bytes),
            Ok(request)
        );

        let mut hostile = bytes;
        hostile[11] = 1;
        assert_eq!(
            SourceReopenGenerationRequestV1::decode(&hostile),
            Err(Error::InvalidCodec)
        );
        let mut hostile = bytes;
        hostile[10] = SourceReopenFamilyV1::OpenRawPage.wire_byte();
        assert_eq!(
            SourceReopenGenerationRequestV1::decode(&hostile),
            Err(Error::InvalidCodec)
        );
        let mut hostile = bytes;
        hostile[242] = 1;
        assert_eq!(
            SourceReopenGenerationRequestV1::decode(&hostile),
            Err(Error::InvalidCodec)
        );
        let mut hostile = bytes;
        hostile[SOURCE_REOPEN_GENERATION_REQUEST_BYTES - 1] = 1;
        assert_eq!(
            SourceReopenGenerationRequestV1::decode(&hostile),
            Err(Error::InvalidCodec)
        );
        assert_eq!(
            SourceReopenGenerationRequestV1::decode(&bytes[..bytes.len() - 1]),
            Err(Error::InvalidCodec)
        );
    }
}
