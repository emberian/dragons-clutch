//! Shared production Series source discovery and finalized-corpus acquisition.
//!
//! The browser and CLI transport opaque native-produced source bytes and
//! finalized account facts. This module owns decoding, address discovery and
//! the deterministic transition into the existing native acquisition operator.
//! It cannot fetch, sign, seed accounts or submit transactions.

use crate::ObservedAccount;
use crate::series_lifecycle_v3::{
    SeriesCurrentOccurrenceV3, SeriesLifecycleReportV3, SeriesLifecycleSnapshotV3, SeriesNextActV3,
    SeriesTerminalTicketV3, inspect_series_lifecycle_v3, series_account_key_v3,
};
use crate::{
    Finality,
    direct_inline_route_v3::{DirectHotFixedRouteV3, FinalizedRecordRouteV3},
    series_current_acquisition_v5::{
        SeriesConsumeShadowObservationsV5, SeriesCurrentAcquisitionInputV5,
        SeriesSelectedRecordObservationsV5, acquire_current_series_hot_v5,
    },
    series_hot_v3::{
        CheckedSeriesShadowAcceleratorV3, SeriesCurrentHotPlanV5, inspect_current_series_hot_v5,
    },
};
use crate::{Observation, series_hot_v3::SeriesSelectedHotReportV5};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_core_contract::ContentId;
use dclutch_market::capability_program::{CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1};
use dclutch_market::execution_strategy::shadow_v3::ShadowRequestV3;
use dclutch_market::{SeriesCoreRequestV1, SeriesPermitExpiryRequestV1};
use dclutch_trading::series::{
    TemplateV3,
    replay::{SERIES_STATE_BYTES_V3, SeriesStateV3, TicketStateV3},
    request::SeriesActionRequestV3,
    terminal::SeriesLifecycleRentSinkV3,
};
use dclutch_trading_sbf::series::{
    account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
    artifacts_v3::{
        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
    expire_funding_artifacts_v5::{
        SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5, SeriesExpireAccountProfileInputV5,
        SeriesExpireChildRequestsV5,
    },
    occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
    prepare_funding_artifacts_v5::{
        SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5, SeriesPrepareAccountProfileInputV5,
    },
    release_v5::{
        SeriesCurrentReleaseInputV5, authenticate_series_selected_action_v5,
        compile_series_release_v5, emit_current_series_release_source_v5,
    },
    template_content_id,
};
use serde::{Deserialize, Serialize};
use solana_program::{hash::hash, pubkey::Pubkey, rent::Rent};
use std::collections::{BTreeMap, BTreeSet};

/// Located refusal while decoding or authenticating a Series source corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesCorpusErrorV1(String);
impl SeriesCorpusErrorV1 {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}
impl std::fmt::Display for SeriesCorpusErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for SeriesCorpusErrorV1 {}
type Error = SeriesCorpusErrorV1;
type Result<T> = std::result::Result<T, SeriesCorpusErrorV1>;
/// Addresses for one finalized Registry record and its vacant staging cursor.
/// No caller-authored privilege is accepted anywhere in the campaign input.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SeriesFinalizedRecordAddressesV2 {
    /// Native raw fact, reauthenticated during acquisition.
    pub raw: String,
    /// Address of its canonically vacant Registry staging cursor.
    pub staging: String,
}

/// Address-only common Hot acquisition coordinates. The operator owns the
/// fixed-coordinate order, privileges, owners, widths, and record admission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SeriesHotFixedAddressesV2 {
    /// Native market fact, reauthenticated during acquisition.
    pub market: String,
    /// Native root fact; reauthenticated before instruction construction.
    pub root: String,
    /// Native manifest fact, reauthenticated during acquisition.
    pub manifest: SeriesFinalizedRecordAddressesV2,
    /// Native program set fact, reauthenticated during acquisition.
    pub program_set: SeriesFinalizedRecordAddressesV2,
    /// Native descriptor fact, reauthenticated during acquisition.
    pub descriptor: SeriesFinalizedRecordAddressesV2,
    /// Native config fact; reauthenticated before instruction construction.
    pub config: SeriesFinalizedRecordAddressesV2,
    /// Native account profile fact, reauthenticated during acquisition.
    pub account_profile: SeriesFinalizedRecordAddressesV2,
    /// Native request profile fact, reauthenticated during acquisition.
    pub request_profile: SeriesFinalizedRecordAddressesV2,
    /// Native transition fact, reauthenticated during acquisition.
    pub transition: SeriesFinalizedRecordAddressesV2,
    /// Native effect fact; reauthenticated before instruction construction.
    pub effect: SeriesFinalizedRecordAddressesV2,
    /// Native lifecycle fact, reauthenticated during acquisition.
    pub lifecycle: SeriesFinalizedRecordAddressesV2,
    /// Native strategy fact, reauthenticated during acquisition.
    pub strategy: SeriesFinalizedRecordAddressesV2,
    /// Native activation cache fact, reauthenticated during acquisition.
    pub activation_cache: String,
    /// Native core program fact; reauthenticated before instruction construction.
    pub core_program: String,
    /// Native core programdata fact, reauthenticated during acquisition.
    pub core_programdata: String,
    /// Native trading program fact, reauthenticated during acquisition.
    pub trading_program: String,
    /// Native trading programdata fact, reauthenticated during acquisition.
    pub trading_programdata: String,
    /// Native registry program fact; reauthenticated before instruction construction.
    pub registry_program: String,
    /// Native rent sysvar fact, reauthenticated during acquisition.
    pub rent_sysvar: String,
    /// Native instructions sysvar fact, reauthenticated during acquisition.
    pub instructions_sysvar: String,
    /// Native product fact, reauthenticated during acquisition.
    pub product: SeriesFinalizedRecordAddressesV2,
    /// Native result domain fact; reauthenticated before instruction construction.
    pub result_domain: SeriesFinalizedRecordAddressesV2,
    /// Native portfolio fact, reauthenticated during acquisition.
    pub portfolio: SeriesFinalizedRecordAddressesV2,
    /// Native linked basis fact, reauthenticated during acquisition.
    pub linked_basis: SeriesFinalizedRecordAddressesV2,
    /// Native capability seal fact, reauthenticated during acquisition.
    pub capability_seal: String,
}

/// Consume-only address/provenance input. Exact records, deployment, request,
/// caller PDA, and checked-manifest identity are reauthenticated by the
/// production acquisition operator; this carries no privilege or alias truth.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SeriesConsumeShadowAcquisitionV2 {
    /// Native certificate fact, reauthenticated during acquisition.
    pub certificate: SeriesFinalizedRecordAddressesV2,
    /// Native artifact fact; reauthenticated before instruction construction.
    pub artifact: SeriesFinalizedRecordAddressesV2,
    /// Native accelerator program fact, reauthenticated during acquisition.
    pub accelerator_program: String,
    /// Native accelerator programdata fact, reauthenticated during acquisition.
    pub accelerator_programdata: String,
    /// Native caller authority fact, reauthenticated during acquisition.
    pub caller_authority: String,
    /// Native checked manifest sha256 fact; reauthenticated before instruction construction.
    pub checked_manifest_sha256: String,
    /// Canonical base64 encoding of the native request request.
    pub request_base64: String,
}

impl SeriesFinalizedRecordAddressesV2 {
    /// Borrow the native-produced record and role addresses needed by acquisition.
    pub fn addresses(&self) -> [&str; 2] {
        [&self.raw, &self.staging]
    }
}

impl SeriesHotFixedAddressesV2 {
    /// Borrow the native-produced record and role addresses needed by acquisition.
    pub fn addresses(&self) -> Vec<&str> {
        let mut output = vec![self.market.as_str(), self.root.as_str()];
        for record in [
            &self.manifest,
            &self.program_set,
            &self.descriptor,
            &self.config,
            &self.account_profile,
            &self.request_profile,
            &self.transition,
            &self.effect,
            &self.lifecycle,
            &self.strategy,
        ] {
            output.extend(record.addresses());
        }
        output.extend([
            self.activation_cache.as_str(),
            self.core_program.as_str(),
            self.core_programdata.as_str(),
            self.trading_program.as_str(),
            self.trading_programdata.as_str(),
            self.registry_program.as_str(),
            self.rent_sysvar.as_str(),
            self.instructions_sysvar.as_str(),
        ]);
        for record in [
            &self.product,
            &self.result_domain,
            &self.portfolio,
            &self.linked_basis,
        ] {
            output.extend(record.addresses());
        }
        output.push(self.capability_seal.as_str());
        output
    }
}

impl SeriesConsumeShadowAcquisitionV2 {
    /// Borrow the native-produced record and role addresses needed by acquisition.
    pub fn addresses(&self) -> [&str; 7] {
        [
            &self.certificate.raw,
            &self.certificate.staging,
            &self.artifact.raw,
            &self.artifact.staging,
            &self.accelerator_program,
            &self.accelerator_programdata,
            &self.caller_authority,
        ]
    }
}

/// Current-occurrence evidence routing. Immutable bodies and replay bytes are
/// read from the same finalized RPC response as the Hot account frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SeriesCurrentOccurrenceRouteV1 {
    /// Native occurrence record fact, reauthenticated during acquisition.
    pub occurrence_record: String,
    /// Native occurrence staging fact; reauthenticated before instruction construction.
    pub occurrence_staging: String,
    /// Native ticket record fact, reauthenticated during acquisition.
    pub ticket_record: String,
    /// Native ticket staging fact, reauthenticated during acquisition.
    pub ticket_staging: String,
    /// Native ticket replay fact, reauthenticated during acquisition.
    pub ticket_replay: Option<String>,
    /// Native siblings fact; reauthenticated before instruction construction.
    pub siblings: Vec<String>,
}

/// Terminal Ticket routing. The planner hostile-decodes both live accounts and
/// proves the replay is terminal before it may select Retire.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SeriesTerminalTicketRouteV1 {
    /// Native ticket record fact, reauthenticated during acquisition.
    pub ticket_record: String,
    /// Native ticket staging fact; reauthenticated before instruction construction.
    pub ticket_staging: String,
    /// Native ticket replay fact, reauthenticated during acquisition.
    pub ticket_replay: String,
}

/// One sequence-indexed acquisition recipe. Future entries are inert candidate
/// addresses, not preauthorized banks: only the current entry can become a
/// durable frame after one finalized observation passes the canonical V5
/// acquisition operator.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SeriesHotAcquisitionRecipeV2 {
    /// Native sequence fact, reauthenticated during acquisition.
    pub sequence: u32,
    /// Native fixed fact; reauthenticated before instruction construction.
    pub fixed: SeriesHotFixedAddressesV2,
    /// Native runtime logical accounts fact, reauthenticated during acquisition.
    pub runtime_logical_accounts: Vec<String>,
    /// Native consume shadow fact, reauthenticated during acquisition.
    pub consume_shadow: Option<SeriesConsumeShadowAcquisitionV2>,
    /// Native current occurrence fact, reauthenticated during acquisition.
    pub current_occurrence: Option<SeriesCurrentOccurrenceRouteV1>,
    /// Native terminal ticket fact; reauthenticated before instruction construction.
    pub terminal_ticket: Option<SeriesTerminalTicketRouteV1>,
    /// Native lifecycle rent credit fact, reauthenticated during acquisition.
    pub lifecycle_rent_credit: Option<String>,
    /// Native expire permit fact, reauthenticated during acquisition.
    pub expire_permit: Option<String>,
}

/// Candidate corpus consumed by the current semantic emitters. These values
/// cannot authorize a release: the production operator requires the emitted
/// ProgramSet, descriptor, ProfileV3, lifecycle, strategy, transition, and
/// EffectV5 bytes to match the live finalized accounts byte-for-byte.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SeriesCurrentSourceCorpusV1 {
    /// Native template occurrence count fact, reauthenticated during acquisition.
    pub template_occurrence_count: u32,
    /// Native consume shadow certificate program fact; reauthenticated before instruction construction.
    pub consume_shadow_certificate_program: String,
    /// Native prepare fixed data lengths fact, reauthenticated during acquisition.
    pub prepare_fixed_data_lengths: Vec<u32>,
    /// Native prepare ticket rent lamports fact, reauthenticated during acquisition.
    pub prepare_ticket_rent_lamports: u64,
    /// Canonical base64 encoding of the native prepare projected initialize request.
    pub prepare_projected_initialize_base64: String,
    /// Canonical base64 for the native prepare projected open request.
    pub prepare_projected_open_base64: String,
    /// Canonical base64 encoding of the native prepare replay initialize request.
    pub prepare_replay_initialize_base64: String,
    /// Canonical base64 encoding of the native prepare escrow open request.
    pub prepare_escrow_open_base64: String,
    /// Canonical base64 encoding of the native prepare escrow lock request.
    pub prepare_escrow_lock_base64: String,
    /// Native consume fixed data lengths fact; reauthenticated before instruction construction.
    pub consume_fixed_data_lengths: Vec<u32>,
    /// Canonical base64 encoding of the native consume lock request.
    pub consume_lock_base64: String,
    /// Canonical base64 encoding of the native consume core request.
    pub consume_core_base64: String,
    /// Canonical base64 encoding of the native consume realize request.
    pub consume_realize_base64: String,
    /// Canonical base64 for the native consume claims request.
    pub consume_claims_base64: String,
    /// Native consume funding count fact, reauthenticated during acquisition.
    pub consume_funding_count: u32,
    /// Native expire fixed data lengths fact, reauthenticated during acquisition.
    pub expire_fixed_data_lengths: Vec<u32>,
    /// Canonical base64 encoding of the native expire refund request.
    pub expire_refund_base64: String,
    /// Canonical base64 for the native expire close vault request.
    pub expire_close_vault_base64: String,
    /// Canonical base64 encoding of the native expire close replay request.
    pub expire_close_replay_base64: String,
    /// Canonical base64 encoding of the native expire projected abort request.
    pub expire_projected_abort_base64: String,
    /// Canonical base64 encoding of the native expire permit expiry request.
    pub expire_permit_expiry_base64: String,
    /// Canonical base64 for the native expire core request.
    pub expire_core_base64: String,
}

/// Host-decoded current-source corpus. Fixed arrays are exact-width so no
/// runtime slice can silently alter one emitter's geometry.
pub struct DecodedSeriesCurrentSourceV1 {
    /// Native template occurrence count fact, reauthenticated during acquisition.
    pub template_occurrence_count: u32,
    /// Native consume shadow certificate program fact, reauthenticated during acquisition.
    pub consume_shadow_certificate_program: ContentId,
    /// Native prepare fixed data lengths fact, reauthenticated during acquisition.
    pub prepare_fixed_data_lengths: [u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
    /// Native prepare ticket rent lamports fact; reauthenticated before instruction construction.
    pub prepare_ticket_rent_lamports: u64,
    /// Native prepare projected initialize fact, reauthenticated during acquisition.
    pub prepare_projected_initialize: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Native prepare projected open fact, reauthenticated during acquisition.
    pub prepare_projected_open: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Native prepare replay initialize fact, reauthenticated during acquisition.
    pub prepare_replay_initialize: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Native prepare escrow open fact; reauthenticated before instruction construction.
    pub prepare_escrow_open: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Native prepare escrow lock fact, reauthenticated during acquisition.
    pub prepare_escrow_lock: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Native consume fixed data lengths fact, reauthenticated during acquisition.
    pub consume_fixed_data_lengths: [u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    /// Native consume lock fact, reauthenticated during acquisition.
    pub consume_lock: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Native consume core fact; reauthenticated before instruction construction.
    pub consume_core: [u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3],
    /// Native consume realize fact, reauthenticated during acquisition.
    pub consume_realize: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Native consume claims fact, reauthenticated during acquisition.
    pub consume_claims: [u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3],
    /// Native consume funding count fact, reauthenticated during acquisition.
    pub consume_funding_count: u32,
    /// Native expire fixed data lengths fact; reauthenticated before instruction construction.
    pub expire_fixed_data_lengths: [u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
    /// Native expire refund fact, reauthenticated during acquisition.
    pub expire_refund: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Native expire close vault fact, reauthenticated during acquisition.
    pub expire_close_vault: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Native expire close replay fact, reauthenticated during acquisition.
    pub expire_close_replay: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    /// Native expire projected abort fact; reauthenticated before instruction construction.
    pub expire_projected_abort: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    /// Native expire permit expiry fact, reauthenticated during acquisition.
    pub expire_permit_expiry: SeriesPermitExpiryRequestV1,
    /// Native expire core fact, reauthenticated during acquisition.
    pub expire_core: SeriesCoreRequestV1,
}

/// One bounded same-finalized RPC acquisition. `accounts` includes every Hot,
/// lifecycle, source-role, routing, and fee-payer key requested by the frame.
pub struct AcquiredSeriesSelectedV1 {
    /// Native observation fact, reauthenticated during acquisition.
    pub observation: Observation,
    /// Native accounts fact, reauthenticated during acquisition.
    pub accounts: BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    /// Native lifecycle fact, reauthenticated during acquisition.
    pub lifecycle: SeriesLifecycleReportV3,
    /// Native authenticated selected action and unsigned instruction.
    pub selected: SeriesSelectedHotReportV5,
}

impl DecodedSeriesCurrentSourceV1 {
    /// Project the validated native release input into the owned source bank.
    pub fn from_release_v1(input: SeriesCurrentReleaseInputV5<'_>) -> Result<Self> {
        if input.consume_funding_count == 0 || input.prepare_ticket_rent_lamports == 0 {
            return Err(refusal(
                "Series Prepare release omitted funding span or Ticket rent",
            ));
        }
        Ok(Self {
            template_occurrence_count: input.template_occurrence_count,
            consume_shadow_certificate_program: input.consume_shadow_certificate_program,
            prepare_fixed_data_lengths: *input.prepare_profile.fixed_data_lengths,
            prepare_ticket_rent_lamports: input.prepare_ticket_rent_lamports,
            prepare_projected_initialize: *input.prepare_requests.projected_initialize,
            prepare_projected_open: *input.prepare_requests.projected_open,
            prepare_replay_initialize: *input.prepare_requests.replay_initialize,
            prepare_escrow_open: *input.prepare_requests.escrow_open,
            prepare_escrow_lock: *input.prepare_requests.escrow_lock,
            consume_fixed_data_lengths: *input.consume_observed_data_lengths,
            consume_lock: *input.consume_requests.lock,
            consume_core: *input.consume_requests.core,
            consume_realize: *input.consume_requests.realize,
            consume_claims: *input.consume_requests.claims,
            consume_funding_count: input.consume_funding_count,
            expire_fixed_data_lengths: *input.expire_profile.fixed_data_lengths,
            expire_refund: *input.expire_requests.refund,
            expire_close_vault: *input.expire_requests.close_vault,
            expire_close_replay: *input.expire_requests.close_replay,
            expire_projected_abort: *input.expire_requests.projected_abort,
            expire_permit_expiry: input.expire_requests.permit_expiry,
            expire_core: input.expire_requests.core_expire,
        })
    }

    /// Hostile-decode a transported candidate bank into exact native request widths.
    pub fn decode(candidate: &SeriesCurrentSourceCorpusV1) -> Result<Self> {
        let consume_shadow_certificate_program = ContentId::new(parse_hex32_v1(
            &candidate.consume_shadow_certificate_program,
            "Series Consume Shadow certificate program",
        )?)
        .map_err(|_| refusal("Series Consume Shadow certificate program was zero"))?;
        let prepare_fixed_data_lengths = candidate
            .prepare_fixed_data_lengths
            .clone()
            .try_into()
            .map_err(|_| refusal("Series Prepare fixed-width corpus changed cardinality"))?;
        let consume_fixed_data_lengths = candidate
            .consume_fixed_data_lengths
            .clone()
            .try_into()
            .map_err(|_| refusal("Series Consume fixed-width corpus changed cardinality"))?;
        let expire_fixed_data_lengths = candidate
            .expire_fixed_data_lengths
            .clone()
            .try_into()
            .map_err(|_| refusal("Series Expire fixed-width corpus changed cardinality"))?;
        let expire_permit_expiry = SeriesPermitExpiryRequestV1::decode(&decode_base64(
            &candidate.expire_permit_expiry_base64,
            "Series Expire permit request",
        )?)
        .map_err(|_| refusal("Series Expire permit request was not canonical"))?;
        let expire_core = SeriesCoreRequestV1::decode(&decode_base64(
            &candidate.expire_core_base64,
            "Series Expire Core request",
        )?)
        .map_err(|_| refusal("Series Expire Core request was not canonical"))?;
        if candidate.consume_funding_count == 0 || candidate.prepare_ticket_rent_lamports == 0 {
            return Err(refusal(
                "Series Consume funding span or Prepare Ticket rent was zero",
            ));
        }
        if candidate.template_occurrence_count == 0 {
            return Err(refusal(
                "Series current-source Template occurrence count was zero",
            ));
        }
        Ok(Self {
            template_occurrence_count: candidate.template_occurrence_count,
            consume_shadow_certificate_program,
            prepare_fixed_data_lengths,
            prepare_ticket_rent_lamports: candidate.prepare_ticket_rent_lamports,
            prepare_projected_initialize: decode_exact_base64_v1(
                &candidate.prepare_projected_initialize_base64,
                "Series Prepare projected initialize",
            )?,
            prepare_projected_open: decode_exact_base64_v1(
                &candidate.prepare_projected_open_base64,
                "Series Prepare projected open",
            )?,
            prepare_replay_initialize: decode_exact_base64_v1(
                &candidate.prepare_replay_initialize_base64,
                "Series Prepare replay initialize",
            )?,
            prepare_escrow_open: decode_exact_base64_v1(
                &candidate.prepare_escrow_open_base64,
                "Series Prepare escrow open",
            )?,
            prepare_escrow_lock: decode_exact_base64_v1(
                &candidate.prepare_escrow_lock_base64,
                "Series Prepare escrow lock",
            )?,
            consume_fixed_data_lengths,
            consume_lock: decode_exact_base64_v1(
                &candidate.consume_lock_base64,
                "Series Consume lock",
            )?,
            consume_core: decode_exact_base64_v1(
                &candidate.consume_core_base64,
                "Series Consume Core",
            )?,
            consume_realize: decode_exact_base64_v1(
                &candidate.consume_realize_base64,
                "Series Consume realize",
            )?,
            consume_claims: decode_exact_base64_v1(
                &candidate.consume_claims_base64,
                "Series Consume Claims",
            )?,
            consume_funding_count: candidate.consume_funding_count,
            expire_fixed_data_lengths,
            expire_refund: decode_exact_base64_v1(
                &candidate.expire_refund_base64,
                "Series Expire refund",
            )?,
            expire_close_vault: decode_exact_base64_v1(
                &candidate.expire_close_vault_base64,
                "Series Expire close vault",
            )?,
            expire_close_replay: decode_exact_base64_v1(
                &candidate.expire_close_replay_base64,
                "Series Expire close replay",
            )?,
            expire_projected_abort: decode_exact_base64_v1(
                &candidate.expire_projected_abort_base64,
                "Series Expire projected abort",
            )?,
            expire_permit_expiry,
            expire_core,
        })
    }

    /// Borrow the canonical emitter input for the selected immutable Template.
    pub fn input(&self, template: ContentId) -> SeriesCurrentReleaseInputV5<'_> {
        SeriesCurrentReleaseInputV5 {
            template,
            template_occurrence_count: self.template_occurrence_count,
            consume_shadow_certificate_program: self.consume_shadow_certificate_program,
            prepare_profile: SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &self.prepare_fixed_data_lengths,
            },
            prepare_requests: SeriesPrepareChildRequestsV4 {
                projected_initialize: &self.prepare_projected_initialize,
                projected_open: &self.prepare_projected_open,
                replay_initialize: &self.prepare_replay_initialize,
                escrow_open: &self.prepare_escrow_open,
                escrow_lock: &self.prepare_escrow_lock,
            },
            prepare_ticket_rent_lamports: self.prepare_ticket_rent_lamports,
            consume_observed_data_lengths: &self.consume_fixed_data_lengths,
            consume_requests: SeriesConsumeChildRequestsV4 {
                lock: &self.consume_lock,
                core: &self.consume_core,
                realize: &self.consume_realize,
                claims: &self.consume_claims,
            },
            consume_funding_count: self.consume_funding_count,
            expire_profile: SeriesExpireAccountProfileInputV5 {
                fixed_data_lengths: &self.expire_fixed_data_lengths,
            },
            expire_requests: SeriesExpireChildRequestsV5 {
                refund: &self.expire_refund,
                close_vault: &self.expire_close_vault,
                close_replay: &self.expire_close_replay,
                projected_abort: &self.expire_projected_abort,
                permit_expiry: self.expire_permit_expiry,
                core_expire: self.expire_core,
            },
        }
    }
}

/// Host account fact captured directly from one finalized RPC observation.
#[derive(Clone, Debug)]
pub struct SeriesObservedAccountV1 {
    /// Native key fact, reauthenticated during acquisition.
    pub key: Pubkey,
    /// Native owner fact, reauthenticated during acquisition.
    pub owner: Pubkey,
    /// Observed account balance in lamports.
    pub lamports: u64,
    /// Native executable fact, reauthenticated during acquisition.
    pub executable: bool,
    /// Native data fact, reauthenticated during acquisition.
    pub data: Vec<u8>,
}

/// A present account or exact absence at one finalized slot.
#[derive(Clone, Debug)]
pub struct SeriesObservedAccountSlotV1 {
    /// Native key fact, reauthenticated during acquisition.
    pub key: Pubkey,
    /// Native account fact, reauthenticated during acquisition.
    pub account: Option<SeriesObservedAccountV1>,
}

fn series_operator_account_from_address_v2(
    accounts: &BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    address: &str,
    label: &str,
    observation: Observation,
) -> Result<ObservedAccount> {
    let key = parse_pubkey(address, label)?;
    let value = accounts
        .get(&key)
        .ok_or_else(|| refusal(format!("{label} was outside the bounded acquisition")))?;
    match value {
        Some(value) => operator_account_v1(value, observation),
        None if key != Pubkey::default() => Ok(ObservedAccount {
            observation,
            key,
            owner: solana_sdk_ids::system_program::ID,
            lamports: 0,
            executable: false,
            data: Vec::new(),
        }),
        None => Err(refusal("Series native System program was absent")),
    }
}

fn series_finalized_record_route_v2(
    accounts: &BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    raw: &str,
    staging: &str,
    label: &str,
    observation: Observation,
) -> Result<FinalizedRecordRouteV3> {
    Ok(FinalizedRecordRouteV3 {
        raw: series_operator_account_from_address_v2(
            accounts,
            raw,
            &format!("{label} raw"),
            observation,
        )?,
        staging: series_operator_account_from_address_v2(
            accounts,
            staging,
            &format!("{label} staging"),
            observation,
        )?,
    })
}

fn series_hot_fixed_route_v2(
    addresses: &SeriesHotFixedAddressesV2,
    accounts: &BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    observation: Observation,
) -> Result<DirectHotFixedRouteV3> {
    let account = |address: &str, label: &str| {
        series_operator_account_from_address_v2(accounts, address, label, observation)
    };
    let record = |addresses: &SeriesFinalizedRecordAddressesV2, label: &str| {
        series_finalized_record_route_v2(
            accounts,
            &addresses.raw,
            &addresses.staging,
            label,
            observation,
        )
    };
    Ok(DirectHotFixedRouteV3 {
        market: account(&addresses.market, "Series controller Market")?,
        root: account(&addresses.root, "Series capability root")?,
        manifest: record(&addresses.manifest, "Series CapabilityManifest")?,
        program_set: record(&addresses.program_set, "Series CapabilityProgramSet")?,
        descriptor: record(&addresses.descriptor, "Series CapabilityProgram")?,
        config: record(&addresses.config, "Series Template")?,
        account_profile: record(&addresses.account_profile, "Series AccountProfile")?,
        request_profile: record(&addresses.request_profile, "Series RequestProfile")?,
        transition: record(&addresses.transition, "Series Transition")?,
        effect: record(&addresses.effect, "Series Effect")?,
        lifecycle: record(&addresses.lifecycle, "Series lifecycle policy")?,
        strategy: record(&addresses.strategy, "Series execution strategy")?,
        activation_cache: account(&addresses.activation_cache, "Series activation cache")?,
        core_program: account(&addresses.core_program, "Series Core program")?,
        core_programdata: account(&addresses.core_programdata, "Series Core ProgramData")?,
        trading_program: account(&addresses.trading_program, "Series Trading program")?,
        trading_programdata: account(&addresses.trading_programdata, "Series Trading ProgramData")?,
        registry_program: account(&addresses.registry_program, "Series Registry program")?,
        rent_sysvar: account(&addresses.rent_sysvar, "Series Rent sysvar")?,
        instructions_sysvar: account(&addresses.instructions_sysvar, "Series Instructions sysvar")?,
        product: record(&addresses.product, "Series Product")?,
        result_domain: record(&addresses.result_domain, "Series result domain")?,
        portfolio: record(&addresses.portfolio, "Series portfolio")?,
        linked_basis: record(&addresses.linked_basis, "Series linked basis")?,
        capability_seal: account(&addresses.capability_seal, "Series capability seal")?,
    })
}

/// Convert an observed present account, admitting zero only for executable native System.
pub fn operator_account_v1(
    value: &SeriesObservedAccountV1,
    observation: Observation,
) -> Result<ObservedAccount> {
    if !is_series_source_address_v1(value.key, Some(value.owner))
        || value.key == Pubkey::default() && !value.executable
    {
        return Err(refusal(
            "Series observed zero key was not the native System program",
        ));
    }
    Ok(ObservedAccount {
        observation,
        key: value.key,
        owner: value.owner,
        lamports: value.lamports,
        executable: value.executable,
        data: value.data.clone(),
    })
}

/// Require a present account while keeping unobserved keys distinct from observed absence.
pub fn required_series_account_v1<'a>(
    accounts: &'a BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
    key: Pubkey,
    label: &str,
) -> Result<&'a SeriesObservedAccountV1> {
    accounts
        .get(&key)
        .ok_or_else(|| refusal(format!("{label} was outside the bounded acquisition")))?
        .as_ref()
        .ok_or_else(|| refusal(format!("{label} was absent")))
}

/// Decode canonical base64 into a native fixed-width request body.
pub fn decode_exact_base64_v1<const N: usize>(value: &str, label: &str) -> Result<[u8; N]> {
    decode_base64(value, label)?
        .try_into()
        .map_err(|_| refusal(format!("{label} changed exact width")))
}

/// Decode one exact 32-byte hexadecimal content identity.
pub fn parse_hex32_v1(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(refusal(format!("{label} was not canonical SHA-256 hex")));
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair =
            std::str::from_utf8(pair).map_err(|_| refusal(format!("{label} was not UTF-8 hex")))?;
        output[index] = u8::from_str_radix(pair, 16)
            .map_err(|_| refusal(format!("{label} was not canonical hex")))?;
    }
    Ok(output)
}

/// Opaque production source emitted by the native constructor and reauthenticated against finalized records.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SeriesNativeOperationSourceV1 {
    /// Native schema fact, reauthenticated during acquisition.
    pub schema: String,
    /// Genesis identity that the transport must match to the live cluster.
    pub genesis_hash: String,
    /// Native payer fact, reauthenticated during acquisition.
    pub payer: String,
    /// Native lookup table fact, reauthenticated during acquisition.
    pub lookup_table: String,
    /// Native lookup table sha256 fact, reauthenticated during acquisition.
    pub lookup_table_sha256: String,
    /// Controller root bound by the source producer.
    pub accepted_root: String,
    /// Canonical base64 encoding of the native accepted request request.
    pub accepted_request_base64: String,
    /// Native current source fact, reauthenticated during acquisition.
    pub current_source: SeriesCurrentSourceCorpusV1,
    /// Native acquisition fact, reauthenticated during acquisition.
    pub acquisition: SeriesHotAcquisitionRecipeV2,
}

impl DecodedSeriesCurrentSourceV1 {
    /// Serialize the native source bank without interpreting request bytes.
    pub fn document(&self) -> Result<SeriesCurrentSourceCorpusV1> {
        Ok(SeriesCurrentSourceCorpusV1 {
            template_occurrence_count: self.template_occurrence_count,
            consume_shadow_certificate_program: hex32(
                self.consume_shadow_certificate_program.to_bytes(),
            ),
            prepare_fixed_data_lengths: self.prepare_fixed_data_lengths.to_vec(),
            prepare_ticket_rent_lamports: self.prepare_ticket_rent_lamports,
            prepare_projected_initialize_base64: BASE64.encode(self.prepare_projected_initialize),
            prepare_projected_open_base64: BASE64.encode(self.prepare_projected_open),
            prepare_replay_initialize_base64: BASE64.encode(self.prepare_replay_initialize),
            prepare_escrow_open_base64: BASE64.encode(self.prepare_escrow_open),
            prepare_escrow_lock_base64: BASE64.encode(self.prepare_escrow_lock),
            consume_fixed_data_lengths: self.consume_fixed_data_lengths.to_vec(),
            consume_lock_base64: BASE64.encode(self.consume_lock),
            consume_core_base64: BASE64.encode(self.consume_core),
            consume_realize_base64: BASE64.encode(self.consume_realize),
            consume_claims_base64: BASE64.encode(self.consume_claims),
            consume_funding_count: self.consume_funding_count,
            expire_fixed_data_lengths: self.expire_fixed_data_lengths.to_vec(),
            expire_refund_base64: BASE64.encode(self.expire_refund),
            expire_close_vault_base64: BASE64.encode(self.expire_close_vault),
            expire_close_replay_base64: BASE64.encode(self.expire_close_replay),
            expire_projected_abort_base64: BASE64.encode(self.expire_projected_abort),
            expire_permit_expiry_base64: BASE64.encode(
                self.expire_permit_expiry.encode().map_err(|error| {
                    refusal(format!("Series permit source encoding: {error:?}"))
                })?,
            ),
            expire_core_base64: BASE64.encode(
                self.expire_core
                    .encode()
                    .map_err(|error| refusal(format!("Series Core source encoding: {error:?}")))?,
            ),
        })
    }
}

/// Discover the exact account keys needed by an opaque native-produced source.
pub fn series_acquisition_addresses_v1(
    frame: &SeriesHotAcquisitionRecipeV2,
    payer: Pubkey,
    lookup_table: Pubkey,
) -> Result<Vec<Pubkey>> {
    let mut keys = BTreeSet::new();
    for address in frame
        .fixed
        .addresses()
        .into_iter()
        .chain(frame.runtime_logical_accounts.iter().map(String::as_str))
        .chain(
            frame
                .consume_shadow
                .iter()
                .flat_map(SeriesConsumeShadowAcquisitionV2::addresses),
        )
    {
        keys.insert(parse_pubkey(address, "Series acquisition account")?);
    }
    if let Some(current) = &frame.current_occurrence {
        for (address, label) in [
            (&current.occurrence_record, "Series occurrence record"),
            (&current.occurrence_staging, "Series occurrence staging"),
            (&current.ticket_record, "Series Ticket record"),
            (&current.ticket_staging, "Series Ticket staging"),
        ] {
            keys.insert(parse_pubkey(address, label)?);
        }
        if let Some(replay) = &current.ticket_replay {
            keys.insert(parse_pubkey(replay, "Series current Ticket replay")?);
        }
    }
    if let Some(terminal) = &frame.terminal_ticket {
        for (address, label) in [
            (&terminal.ticket_record, "Series terminal Ticket record"),
            (&terminal.ticket_staging, "Series terminal Ticket staging"),
            (&terminal.ticket_replay, "Series terminal Ticket replay"),
        ] {
            keys.insert(parse_pubkey(address, label)?);
        }
    }
    for candidate in [
        frame.lifecycle_rent_credit.as_ref(),
        frame.expire_permit.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        keys.insert(parse_pubkey(
            candidate,
            "Series lifecycle acquisition role",
        )?);
    }
    keys.insert(payer);
    keys.insert(lookup_table);
    if keys.is_empty() || keys.len() > 512 {
        return Err(refusal(
            "Series acquisition account set was empty or exceeded 512",
        ));
    }
    Ok(keys.into_iter().collect())
}

/// Native lifecycle discovery, schedule wait, or fully authenticated selected action.
pub enum SeriesCorpusPlanV1 {
    /// Additional immutable or replay evidence required by the native planner.
    Acquire(crate::series_lifecycle_v3::SeriesAcquisitionV3),
    /// The next Consume is valid only at or after its authored start slot.
    WaitUntil {
        /// Native occurrence start slot.
        scheduled_slot: u64,
    },
    /// Fully authenticated unsigned action and its finalized corpus.
    Ready(AcquiredSeriesSelectedV1),
}

/// Decode observed Series replay under its canonical root header and Template.
/// This never substitutes activation state for an already finalized root.
pub fn decode_series_root_replay_v1(
    root: &ObservedAccount,
    trading: Pubkey,
    template_bytes: &[u8],
) -> Result<SeriesStateV3> {
    if root.observation.finality != Finality::Finalized
        || root.observation.slot == 0
        || root.owner != trading
        || root.executable
        || trading == Pubkey::default()
    {
        return Err(refusal(
            "Series replay root was not a finalized Trading-owned account",
        ));
    }
    let template = TemplateV3::decode(template_bytes)
        .map_err(|_| refusal("Series live Template refused hostile decode"))?;
    let header = CapabilityRootHeaderV1::decode(
        root.data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or_else(|| refusal("Series composite root omitted its canonical header"))?,
    )
    .map_err(|_| refusal("Series composite root header refused hostile decode"))?;
    if header.selection().config().to_bytes() != hash(template_bytes).to_bytes()
        || header.release_set() != template.release_set()
        || Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0 != root.key
    {
        return Err(refusal(
            "Series composite root changed its Template, release or canonical address",
        ));
    }
    let root_tail = root
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or_else(|| refusal("Series composite root omitted its replay tail"))?;
    if root_tail.len() != SERIES_STATE_BYTES_V3 {
        return Err(refusal("Series composite root replay tail changed width"));
    }
    SeriesStateV3::decode(root_tail, template.occurrence_count())
        .map_err(|_| refusal("Series live replay tail refused hostile decode"))
}

/// Authenticate one complete finalized corpus through the current native Series owners.
pub fn inspect_current_series_corpus_v1(
    frame: &SeriesHotAcquisitionRecipeV2,
    source: &DecodedSeriesCurrentSourceV1,
    observation: Observation,
    accounts: BTreeMap<Pubkey, Option<SeriesObservedAccountV1>>,
) -> Result<SeriesCorpusPlanV1> {
    if observation.finality != Finality::Finalized || observation.slot == 0 {
        return Err(refusal(
            "Series corpus requires a nonzero finalized observation",
        ));
    }
    for (key, value) in &accounts {
        if value.as_ref().is_some_and(|account| account.key != *key) {
            return Err(refusal(
                "Series corpus map key changed its account identity",
            ));
        }
    }
    let fixed = series_hot_fixed_route_v2(&frame.fixed, &accounts, observation)?;
    let template_bytes = fixed.config.raw.data.clone();
    let template = TemplateV3::decode(&template_bytes)
        .map_err(|_| refusal("Series live Template refused hostile decode"))?;
    let template_id = template_content_id(&template_bytes)
        .map_err(|_| refusal("Series live Template identity refused"))?;
    let series =
        decode_series_root_replay_v1(&fixed.root, fixed.trading_program.key, &template_bytes)?;
    let root_lamports = fixed.root.lamports;
    let root_data_len = fixed.root.data.len();
    let rent: Rent = bincode::deserialize(&fixed.rent_sysvar.data)
        .map_err(|_| refusal("Series same-slot Rent sysvar refused decode"))?;
    let siblings = frame
        .current_occurrence
        .as_ref()
        .map(|current| {
            current
                .siblings
                .iter()
                .map(|value| parse_hex32_v1(value, "Series occurrence sibling"))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();
    let current = frame
        .current_occurrence
        .as_ref()
        .map(|route| -> Result<SeriesCurrentOccurrenceV3<'_>> {
            let occurrence = required_series_account_v1(
                &accounts,
                parse_pubkey(&route.occurrence_record, "Series occurrence record")?,
                "Series occurrence record",
            )?;
            let ticket = required_series_account_v1(
                &accounts,
                parse_pubkey(&route.ticket_record, "Series Ticket record")?,
                "Series Ticket record",
            )?;
            let ticket_state = route
                .ticket_replay
                .as_ref()
                .map(|key| -> Result<Option<TicketStateV3>> {
                    let key = parse_pubkey(key, "Series current Ticket replay")?;
                    accounts
                        .get(&key)
                        .ok_or_else(|| refusal("Series current Ticket replay was not observed"))?
                        .as_ref()
                        .map(|account| {
                            TicketStateV3::decode(&account.data).map_err(|_| {
                                refusal("Series current Ticket replay refused hostile decode")
                            })
                        })
                        .transpose()
                })
                .transpose()?
                .flatten();
            Ok(SeriesCurrentOccurrenceV3 {
                occurrence_bytes: &occurrence.data,
                ticket_bytes: &ticket.data,
                siblings: &siblings,
                ticket_state,
            })
        })
        .transpose()?;
    let terminal_ticket = frame
        .terminal_ticket
        .as_ref()
        .map(|route| -> Result<SeriesTerminalTicketV3<'_>> {
            let ticket = required_series_account_v1(
                &accounts,
                parse_pubkey(&route.ticket_record, "Series terminal Ticket record")?,
                "Series terminal Ticket record",
            )?;
            let replay = required_series_account_v1(
                &accounts,
                parse_pubkey(&route.ticket_replay, "Series terminal Ticket replay")?,
                "Series terminal Ticket replay",
            )?;
            let ticket_state = TicketStateV3::decode(&replay.data)
                .map_err(|_| refusal("Series terminal Ticket replay refused hostile decode"))?;
            Ok(SeriesTerminalTicketV3 {
                ticket_bytes: &ticket.data,
                ticket_state,
                observed_lamports: replay.lamports,
                exact_rent: rent.minimum_balance(replay.data.len()),
            })
        })
        .transpose()?;
    let rent_sink = frame
        .lifecycle_rent_credit
        .as_ref()
        .map(|key| -> Result<SeriesLifecycleRentSinkV3> {
            let credit_key = parse_pubkey(key, "Series lifecycle RentCredit")?;
            let credit =
                required_series_account_v1(&accounts, credit_key, "Series lifecycle RentCredit")?;
            let header = CapabilityRootHeaderV1::decode(
                fixed
                    .root
                    .data
                    .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
                    .ok_or_else(|| refusal("Series root omitted its immutable header"))?,
            )
            .map_err(|_| refusal("Series root header refused hostile decode"))?;
            SeriesLifecycleRentSinkV3::admit(
                series_account_key_v3(credit_key.to_bytes())
                    .map_err(|_| refusal("Series RentCredit key refused"))?,
                &credit.data,
                series_account_key_v3(header.market())
                    .map_err(|_| refusal("Series parent Market key refused"))?,
                header.release_set(),
                header.generation(),
                template.refund_owner(),
            )
            .map_err(|_| refusal("Series lifecycle RentCredit refused root/Template binding"))
        })
        .transpose()?;
    let lifecycle_snapshot = SeriesLifecycleSnapshotV3 {
        template_bytes: &template_bytes,
        series,
        now_slot: observation.slot,
        current,
        terminal_ticket,
        observed_root_lamports: root_lamports,
        exact_root_rent: rent.minimum_balance(root_data_len),
        rent_sink,
    };
    let lifecycle = inspect_series_lifecycle_v3(lifecycle_snapshot)
        .map_err(|error| refusal(format!("Series lifecycle planner: {error:?}")))?;
    if source.prepare_ticket_rent_lamports
        != rent.minimum_balance(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3)
    {
        return Err(refusal(
            "Series current-source Ticket rent differed from same-slot Rent",
        ));
    }
    // The count is release GEOMETRY: every occurrence action's family request
    // is exactly `series_action_request_bytes_v3(count)` wide and its Effect
    // declares a borrowed proof range only when that width exceeds the header.
    // The corpus states the count the candidate artifacts were compiled for;
    // the live finalized Template is its only author, so a disagreement is
    // named here rather than discovered as an artifact-shaped refusal later.
    if source.template_occurrence_count != template.occurrence_count() {
        return Err(refusal(format!(
            "Series current-source occurrence count {} differed from the live Template's {}",
            source.template_occurrence_count,
            template.occurrence_count(),
        )));
    }
    let current_source = source.input(template_id);
    let planned = match lifecycle.next() {
        SeriesNextActV3::Ready(planned) => planned,
        SeriesNextActV3::Acquire(needed) => {
            return Ok(SeriesCorpusPlanV1::Acquire(needed));
        }
        SeriesNextActV3::WaitUntil { scheduled_slot } => {
            return Ok(SeriesCorpusPlanV1::WaitUntil { scheduled_slot });
        }
    };
    let owned_release = emit_current_series_release_source_v5(current_source)
        .map_err(|error| refusal(format!("emit current Series V5 release: {error:?}")))?;
    let release = compile_series_release_v5(owned_release.as_source())
        .map_err(|error| refusal(format!("compile current Series V5 release: {error:?}")))?;
    let preselected = authenticate_series_selected_action_v5(
        &release,
        owned_release.as_source(),
        planned.request().as_bytes(),
    )
    .map_err(|error| refusal(format!("authenticate current Series V5 action: {error:?}")))?;
    if preselected.action != planned.action() {
        return Err(refusal(
            "current Series release changed the lifecycle-selected action",
        ));
    }

    let runtime_logical_accounts = frame
        .runtime_logical_accounts
        .iter()
        .map(|address| {
            series_operator_account_from_address_v2(
                &accounts,
                address,
                "Series logical runtime account",
                observation,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let occurrence_record = frame
        .current_occurrence
        .as_ref()
        .map(|route| {
            series_finalized_record_route_v2(
                &accounts,
                &route.occurrence_record,
                &route.occurrence_staging,
                "Series occurrence",
                observation,
            )
        })
        .transpose()?;
    if frame.current_occurrence.is_some() && frame.terminal_ticket.is_some() {
        return Err(refusal(
            "Series acquisition supplied current and terminal Ticket routes together",
        ));
    }
    let ticket_record = match (&frame.current_occurrence, &frame.terminal_ticket) {
        (Some(route), None) => Some(series_finalized_record_route_v2(
            &accounts,
            &route.ticket_record,
            &route.ticket_staging,
            "Series current Ticket",
            observation,
        )?),
        (None, Some(route)) => Some(series_finalized_record_route_v2(
            &accounts,
            &route.ticket_record,
            &route.ticket_staging,
            "Series terminal Ticket",
            observation,
        )?),
        (None, None) => None,
        (Some(_), Some(_)) => unreachable!("refused above"),
    };
    let rent_credit = frame
        .lifecycle_rent_credit
        .as_ref()
        .map(|address| {
            series_operator_account_from_address_v2(
                &accounts,
                address,
                "Series lifecycle RentCredit",
                observation,
            )
        })
        .transpose()?;
    let expire_permit = frame
        .expire_permit
        .as_ref()
        .map(|address| {
            series_operator_account_from_address_v2(
                &accounts,
                address,
                "Series Expire permit",
                observation,
            )
        })
        .transpose()?;
    let shadow_request_bytes = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| decode_base64(&shadow.request_base64, "Series Consume Shadow request"))
        .transpose()?;
    let shadow_request = shadow_request_bytes
        .as_deref()
        .map(ShadowRequestV3::decode)
        .transpose()
        .map_err(|_| refusal("Series Consume Shadow request refused hostile decode"))?;
    let shadow_certificate = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_finalized_record_route_v2(
                &accounts,
                &shadow.certificate.raw,
                &shadow.certificate.staging,
                "Series Consume Shadow certificate",
                observation,
            )
        })
        .transpose()?;
    let shadow_artifact = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_finalized_record_route_v2(
                &accounts,
                &shadow.artifact.raw,
                &shadow.artifact.staging,
                "Series Consume Shadow artifact",
                observation,
            )
        })
        .transpose()?;
    let shadow_accelerator_program = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_operator_account_from_address_v2(
                &accounts,
                &shadow.accelerator_program,
                "Series Consume accelerator program",
                observation,
            )
        })
        .transpose()?;
    let shadow_accelerator_programdata = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_operator_account_from_address_v2(
                &accounts,
                &shadow.accelerator_programdata,
                "Series Consume accelerator ProgramData",
                observation,
            )
        })
        .transpose()?;
    let shadow_caller_authority = frame
        .consume_shadow
        .as_ref()
        .map(|shadow| {
            series_operator_account_from_address_v2(
                &accounts,
                &shadow.caller_authority,
                "Series Consume Shadow caller authority",
                observation,
            )
        })
        .transpose()?;
    let shadow_checked = match (&frame.consume_shadow, &shadow_artifact) {
        (Some(shadow), Some(artifact)) => Some(CheckedSeriesShadowAcceleratorV3 {
            artifact_release: hash(&artifact.raw.data).to_bytes(),
            accelerator_program: shadow_accelerator_program
                .as_ref()
                .ok_or_else(|| refusal("Series Consume accelerator program was absent"))?
                .key,
            accelerator_programdata: shadow_accelerator_programdata
                .as_ref()
                .ok_or_else(|| refusal("Series Consume accelerator ProgramData was absent"))?
                .key,
            checked_manifest_digest: parse_hex32_v1(
                &shadow.checked_manifest_sha256,
                "Series Consume checked manifest",
            )?,
        }),
        (None, None) => None,
        _ => return Err(refusal("Series Consume Shadow acquisition was incomplete")),
    };
    let shadow = match frame.consume_shadow.as_ref() {
        Some(_) => Some(SeriesConsumeShadowObservationsV5 {
            certificate: shadow_certificate
                .as_ref()
                .ok_or_else(|| refusal("Series Consume Shadow certificate was absent"))?,
            artifact: shadow_artifact
                .as_ref()
                .ok_or_else(|| refusal("Series Consume Shadow artifact was absent"))?,
            accelerator_program: shadow_accelerator_program
                .as_ref()
                .ok_or_else(|| refusal("Series Consume accelerator program was absent"))?,
            accelerator_programdata: shadow_accelerator_programdata
                .as_ref()
                .ok_or_else(|| refusal("Series Consume accelerator ProgramData was absent"))?,
            caller_authority: shadow_caller_authority
                .as_ref()
                .ok_or_else(|| refusal("Series Consume Shadow caller authority was absent"))?,
            checked: shadow_checked
                .ok_or_else(|| refusal("Series Consume checked release was absent"))?,
            request: shadow_request
                .ok_or_else(|| refusal("Series Consume Shadow request was absent"))?,
        }),
        None => None,
    };
    let acquired = acquire_current_series_hot_v5(
        &preselected,
        owned_release.action_artifacts(preselected.action),
        SeriesCurrentAcquisitionInputV5 {
            fixed: &fixed,
            runtime_logical_accounts: &runtime_logical_accounts,
            records: SeriesSelectedRecordObservationsV5 {
                occurrence: occurrence_record.as_ref(),
                ticket: ticket_record.as_ref(),
                rent_credit: rent_credit.as_ref(),
                expire_permit: expire_permit.as_ref(),
            },
            shadow,
            lifecycle: lifecycle_snapshot,
        },
    )
    .map_err(|error| refusal(format!("acquire current Series V5 bank: {error:?}")))?;
    let selected = match inspect_current_series_hot_v5(&acquired.state, current_source)
        .map_err(|error| refusal(format!("current Series V5 operator: {error:?}")))?
    {
        SeriesCurrentHotPlanV5::Ready(report) => report,
        SeriesCurrentHotPlanV5::Acquire(needed) => {
            return Ok(SeriesCorpusPlanV1::Acquire(needed));
        }
        SeriesCurrentHotPlanV5::WaitUntil { scheduled_slot } => {
            return Ok(SeriesCorpusPlanV1::WaitUntil { scheduled_slot });
        }
    };
    if selected.observation != observation || selected.selected != preselected {
        return Err(refusal(
            "Series selected report changed the acquired observation or release action",
        ));
    }
    Ok(SeriesCorpusPlanV1::Ready(AcquiredSeriesSelectedV1 {
        observation,
        accounts,
        lifecycle,
        selected,
    }))
}
fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn decode_base64(value: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label}: {error}")))?;
    if BASE64.encode(&bytes) != value {
        return Err(refusal(format!("{label} was not canonical base64")));
    }
    Ok(bytes)
}

fn hex32(value: [u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(format!("REFUSED Series terminal: {}", message.into()))
}

/// Exact schema of the opaque source emitted by the production Rust driver.
pub const SERIES_NATIVE_OPERATION_SOURCE_SCHEMA_V1: &str = "dclutch-series-operation-source-v1";

/// Root/Market intent plus transport facts; no browser-authored account roles
/// or semantic request bodies enter this aggregate.
pub struct SeriesOperationCorpusInputV1<'a> {
    /// Market selected in the Workbench or CLI.
    pub market: Pubkey,
    /// Its Series controller root selected by the user.
    pub root: Pubkey,
    /// Wallet intended to pay the outer transaction fee.
    pub payer: Pubkey,
    /// Genesis fetched by the transport from the admitted RPC origin.
    pub actual_genesis_hash: &'a str,
    /// One finalized observation shared by every supplied account row.
    pub observation: Observation,
    /// Explicit present/absent account results from that one observation.
    pub accounts: &'a [SeriesObservedAccountSlotV1],
    /// Opaque native-produced operation source; its contents remain untrusted
    /// until native emission and finalized-record authentication agree.
    pub native_operation_source: Option<&'a [u8]>,
}

/// Authenticated frozen lookup routing supplied to wallet transport.
pub struct SeriesFrozenLookupV1 {
    /// Exact on-chain lookup table identity.
    pub key: Pubkey,
    /// Ordered addresses authenticated from the finalized account bytes.
    pub addresses: Vec<Pubkey>,
    /// Digest binding the exact frozen account data.
    pub data_sha256: [u8; 32],
}

/// Authenticate immutable, activated Series lookup routing from a finalized account.
pub fn authenticate_series_lookup_table_v1(
    table: &ObservedAccount,
    expected_key: Pubkey,
    expected_sha256: &str,
) -> Result<SeriesFrozenLookupV1> {
    use solana_address_lookup_table_interface::{program, state::AddressLookupTable};
    let expected = parse_hex32_v1(expected_sha256, "Series lookup table")?;
    let decoded = AddressLookupTable::deserialize(&table.data)
        .map_err(|_| refusal("Series lookup table bytes did not decode"))?;
    if table.key != expected_key
        || table.owner != program::id()
        || table.executable
        || decoded.meta.authority.is_some()
        || decoded.meta.deactivation_slot != u64::MAX
        || decoded.meta.last_extended_slot >= table.observation.slot
        || decoded.addresses.is_empty()
        || hash(&table.data).to_bytes() != expected
    {
        return Err(refusal(
            "Series lookup table was not the exact frozen activated routing table",
        ));
    }
    Ok(SeriesFrozenLookupV1 {
        key: table.key,
        addresses: decoded.addresses.to_vec(),
        data_sha256: expected,
    })
}

/// Native discovery or an unsigned action; a transport never chooses the next
/// Series action by assembling its own frame.
pub enum SeriesOperationInspectionV1 {
    /// The native authoring producer must supply an opaque source artifact.
    NeedSource {
        /// User-selected Market needing a native source.
        market: Pubkey,
        /// User-selected Series root needing a native source.
        root: Pubkey,
    },
    /// Fetch these keys together at one finalized observation and call again.
    NeedAccounts {
        /// Complete native-derived acquisition vector, including already seen
        /// keys so the transport does not splice different finalized slots.
        keys: Vec<Pubkey>,
    },
    /// Additional occurrence/proof/replay/RentCredit evidence needed by native lifecycle selection.
    Acquire(crate::series_lifecycle_v3::SeriesAcquisitionV3),
    /// Consume is not ready until the authored native start slot.
    WaitUntil {
        /// First admissible Consume slot.
        scheduled_slot: u64,
    },
    /// Native selected instruction and accepted target/consequence for wallet review.
    Ready {
        /// Exact opaque source digest to bind a subsequent wallet interaction.
        source_sha256: [u8; 32],
        /// Immutable target and consequence bound to the native result.
        intent: crate::series_intent_v1::SeriesOperationIntentV1,
        /// Canonical native unsigned instruction, roles and finalized observation.
        report: SeriesSelectedHotReportV5,
        /// Exact finalized frozen routing, without browser-authored addresses.
        lookup_table: SeriesFrozenLookupV1,
        /// Native hot entrypoint heap request for transaction construction.
        heap_frame_bytes: u32,
    },
}

/// Discover or construct a Series operation using the same deterministic
/// corpus adapter as the production CLI. This function never performs I/O.
pub fn inspect_series_operation_v1(
    input: SeriesOperationCorpusInputV1<'_>,
) -> Result<SeriesOperationInspectionV1> {
    if [input.market, input.root, input.payer].contains(&Pubkey::default()) {
        return Err(refusal(
            "Series operation intent named a default Market, root or payer",
        ));
    }
    let Some(bytes) = input.native_operation_source else {
        return Ok(SeriesOperationInspectionV1::NeedSource {
            market: input.market,
            root: input.root,
        });
    };
    let source: SeriesNativeOperationSourceV1 = serde_json::from_slice(bytes)
        .map_err(|error| refusal(format!("Series native operation source decoding: {error}")))?;
    if source.schema != SERIES_NATIVE_OPERATION_SOURCE_SCHEMA_V1 || source.acquisition.sequence != 0
    {
        return Err(refusal(
            "Series native operation source schema or sequence changed",
        ));
    }
    if source.genesis_hash != input.actual_genesis_hash {
        return Err(refusal("Series native operation source genesis changed"));
    }
    parse_pubkey(input.actual_genesis_hash, "Series operation genesis")?;
    if parse_pubkey(&source.acquisition.fixed.market, "Series source Market")? != input.market
        || parse_pubkey(&source.acquisition.fixed.root, "Series source root")? != input.root
        || parse_pubkey(&source.accepted_root, "Series accepted root")? != input.root
        || parse_pubkey(&source.payer, "Series source payer")? != input.payer
    {
        return Err(refusal(
            "Series native operation source changed the intended Market, root or payer",
        ));
    }
    let lookup = parse_pubkey(&source.lookup_table, "Series lookup table")?;
    let keys = series_acquisition_addresses_v1(&source.acquisition, input.payer, lookup)?;
    if input.accounts.len() > keys.len() {
        return Err(refusal(
            "Series corpus exceeded its native acquisition vector",
        ));
    }
    let mut accounts = BTreeMap::new();
    for row in input.accounts {
        if !keys.contains(&row.key) {
            return Err(refusal(
                "Series corpus included an account outside native acquisition",
            ));
        }
        if row
            .account
            .as_ref()
            .is_some_and(|account| account.key != row.key)
        {
            return Err(refusal("Series corpus row changed its account identity"));
        }
        if accounts.insert(row.key, row.account.clone()).is_some() {
            return Err(refusal("Series corpus repeated an account observation"));
        }
    }
    if keys.iter().any(|key| !accounts.contains_key(key)) {
        return Ok(SeriesOperationInspectionV1::NeedAccounts { keys });
    }
    let observed_lookup = operator_account_v1(
        required_series_account_v1(&accounts, lookup, "Series lookup table")?,
        input.observation,
    )?;
    let lookup_table =
        authenticate_series_lookup_table_v1(&observed_lookup, lookup, &source.lookup_table_sha256)?;
    let decoded = DecodedSeriesCurrentSourceV1::decode(&source.current_source)?;
    let accepted = decode_base64(&source.accepted_request_base64, "Series accepted request")?;
    let accepted = SeriesActionRequestV3::decode(&accepted)
        .map_err(|error| refusal(format!("Series accepted request: {error:?}")))?;
    let intent = crate::series_intent_v1::SeriesOperationIntentV1::from_request(
        input.root.to_bytes(),
        accepted,
    )
    .map_err(|error| refusal(format!("Series source intent: {error:?}")))?;
    match inspect_current_series_corpus_v1(
        &source.acquisition,
        &decoded,
        input.observation,
        accounts,
    )? {
        SeriesCorpusPlanV1::Acquire(needed) => Ok(SeriesOperationInspectionV1::Acquire(needed)),
        SeriesCorpusPlanV1::WaitUntil { scheduled_slot } => {
            Ok(SeriesOperationInspectionV1::WaitUntil { scheduled_slot })
        }
        SeriesCorpusPlanV1::Ready(acquired) => {
            let request = SeriesActionRequestV3::decode(&acquired.selected.selected.request_bytes)
                .map_err(|error| refusal(format!("Series selected request: {error:?}")))?;
            intent
                .require_matches(acquired.selected.roles.root.to_bytes(), request)
                .map_err(|error| {
                    refusal(format!("Series fresh native intent changed: {error:?}"))
                })?;
            Ok(SeriesOperationInspectionV1::Ready {
                source_sha256: hash(bytes).to_bytes(),
                intent,
                report: acquired.selected,
                lookup_table,
                heap_frame_bytes:
                    dclutch_market::capability_program::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> Observation {
        Observation {
            slot: 12,
            unix_timestamp: 0,
            finality: Finality::Finalized,
        }
    }

    #[test]
    fn finalized_root_keeps_second_occurrence_replay_and_refuses_another_owner() {
        use dclutch_market::capability_program::SelectedRecordBumpsV1;
        use dclutch_registry::release_set::CapabilityExecutionSelectionV1;
        let bytes = dclutch_trading::series::generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let template = TemplateV3::decode(&bytes).expect("Template");
        let before = SeriesStateV3::new(template.close_rent())
            .prepare_ticket(0)
            .expect("Prepare")
            .settle_current(1, template.occurrence_count())
            .expect("settle first");
        let selection = CapabilityExecutionSelectionV1::from_bytes(
            0,
            [41; 32],
            [42; 32],
            [43; 32],
            hash(&bytes).to_bytes(),
        )
        .expect("selection");
        let header = CapabilityRootHeaderV1::new(
            template.release_set(),
            [44; 32],
            1,
            selection,
            SelectedRecordBumpsV1::default(),
        )
        .expect("header");
        let trading = Pubkey::new_unique();
        let mut data = header.to_bytes().to_vec();
        data.extend_from_slice(&before.encode(template.occurrence_count()).expect("replay"));
        let mut root = ObservedAccount {
            observation: observation(),
            key: Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0,
            owner: trading,
            lamports: 19,
            executable: false,
            data,
        };
        let decoded =
            decode_series_root_replay_v1(&root, trading, &bytes).expect("observed replay");
        assert_eq!(decoded, before);
        assert_eq!(decoded.next_occurrence(), 1);
        assert_eq!(decoded.current_ticket_prepared(), false);
        root.owner = Pubkey::new_unique();
        assert_eq!(
            decode_series_root_replay_v1(&root, trading, &bytes)
                .expect_err("wrong owner")
                .to_string(),
            "REFUSED Series terminal: Series replay root was not a finalized Trading-owned account"
        );
    }

    #[test]
    fn native_lookup_rejects_mutability_and_preserves_authenticated_order() {
        use solana_address_lookup_table_interface::{
            program,
            state::{AddressLookupTable, LookupTableMeta},
        };
        let keys = vec![Pubkey::new_unique(), Pubkey::new_unique()];
        let make = |authority| {
            let table = AddressLookupTable {
                meta: LookupTableMeta {
                    authority,
                    last_extended_slot: 11,
                    ..LookupTableMeta::default()
                },
                addresses: std::borrow::Cow::Owned(keys.clone()),
            };
            ObservedAccount {
                observation: observation(),
                key: Pubkey::new_unique(),
                owner: program::id(),
                lamports: 1,
                executable: false,
                data: table.serialize_for_tests().expect("table bytes"),
            }
        };
        let frozen = make(None);
        let decoded = authenticate_series_lookup_table_v1(
            &frozen,
            frozen.key,
            &hex32(hash(&frozen.data).to_bytes()),
        )
        .expect("frozen activated table");
        assert_eq!(decoded.addresses, keys);
        let mutable = make(Some(Pubkey::new_unique()));
        let error = authenticate_series_lookup_table_v1(
            &mutable,
            mutable.key,
            &hex32(hash(&mutable.data).to_bytes()),
        )
        .err()
        .expect("mutable routing refused");
        assert_eq!(
            error.to_string(),
            "REFUSED Series terminal: Series lookup table was not the exact frozen activated routing table"
        );
    }

    #[test]
    fn corpus_preserves_observed_vacancy_and_accepts_only_native_system_at_zero() {
        let vacant = Pubkey::new_unique();
        let mut accounts = BTreeMap::from([(vacant, None)]);
        let account = series_operator_account_from_address_v2(
            &accounts,
            &vacant.to_string(),
            "future native PDA",
            observation(),
        )
        .expect("explicit RPC absence remains native vacancy evidence");
        assert_eq!(account.key, vacant);
        assert_eq!(account.owner, solana_sdk_ids::system_program::ID);
        assert_eq!(account.lamports, 0);
        assert!(!account.executable);
        assert!(account.data.is_empty());
        assert_eq!(
            series_operator_account_from_address_v2(
                &accounts,
                &Pubkey::new_unique().to_string(),
                "missing",
                observation()
            )
            .expect_err("unobserved is not absent")
            .to_string(),
            "REFUSED Series terminal: missing was outside the bounded acquisition"
        );
        let system = solana_sdk_ids::system_program::ID;
        accounts.insert(
            system,
            Some(SeriesObservedAccountV1 {
                key: system,
                owner: solana_sdk_ids::native_loader::ID,
                lamports: 1,
                executable: true,
                data: Vec::new(),
            }),
        );
        series_operator_account_from_address_v2(
            &accounts,
            &system.to_string(),
            "System",
            observation(),
        )
        .expect("executable native System");
        accounts
            .get_mut(&system)
            .expect("system slot")
            .as_mut()
            .expect("system")
            .owner = solana_sdk_ids::system_program::ID;
        assert_eq!(
            series_operator_account_from_address_v2(
                &accounts,
                &system.to_string(),
                "System",
                observation()
            )
            .expect_err("wrong owner at zero")
            .to_string(),
            "REFUSED Series terminal: Series observed zero key was not the native System program"
        );
        accounts.insert(system, None);
        assert_eq!(
            series_operator_account_from_address_v2(
                &accounts,
                &system.to_string(),
                "System",
                observation()
            )
            .expect_err("System cannot be absent")
            .to_string(),
            "REFUSED Series terminal: Series native System program was absent"
        );
    }

    #[test]
    fn root_intent_without_native_source_reports_discovery_without_inventing_a_frame() {
        let market = Pubkey::new_unique();
        let root = Pubkey::new_unique();
        let result = inspect_series_operation_v1(SeriesOperationCorpusInputV1 {
            market,
            root,
            payer: Pubkey::new_unique(),
            actual_genesis_hash: "",
            observation: observation(),
            accounts: &[],
            native_operation_source: None,
        })
        .expect("native source discovery");
        assert!(
            matches!(result, SeriesOperationInspectionV1::NeedSource { market: actual_market, root: actual_root }
            if actual_market == market && actual_root == root)
        );
    }
}

/// Shared Series source-address rule: zero is reserved for native System
/// observed with NativeLoader ownership, never a predicted future PDA.
pub fn is_series_source_address_v1(address: Pubkey, expected_owner: Option<Pubkey>) -> bool {
    address != Pubkey::default()
        || address == solana_sdk_ids::system_program::ID
            && expected_owner == Some(solana_sdk_ids::native_loader::ID)
}
