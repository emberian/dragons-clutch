//! Bundle assembly: the fixed Hot frame, the adoption loop, and the final
//! instruction and account set.

use dclutch_account_profile_contract::{
    v2::{AccountProfileV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2},
    v3::{AccountProfileV3, SCHEMA_RELEASE_ID_V3 as ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V3},
};
use dclutch_capability_program_contract::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
    HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3,
    HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
    HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_DESCRIPTOR_STAGING_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
    HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_EXECUTION_ENVELOPE_BYTES_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
    HOT_LIFECYCLE_STAGING_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    HOT_LINKED_BASIS_STAGING_ACCOUNT_V3, HOT_MANIFEST_RAW_ACCOUNT_V3,
    HOT_MANIFEST_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
    HOT_PORTFOLIO_STAGING_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PRODUCT_STAGING_ACCOUNT_V3,
    HOT_PROGRAM_SET_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
    HOT_REGISTRY_PROGRAM_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3,
    HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3, HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
    HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
    HOT_STRATEGY_STAGING_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
    HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3,
    HOT_TRANSITION_STAGING_ACCOUNT_V3, HotBumpHintsV1, HotExecutionEnvelopeV3,
};
use dclutch_capability_program_contract::v4::CapabilityProgramV4;
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_custody_contract::CustodyAuthoritySeedsV1;
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2};
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    BuilderError, WaistFactsV1,
    admitted::{
        AdmittedAotInputV1, AdmittedAuthorityInputV1, DerivedAdmittedAuthoritiesV1,
        DerivedAdmittedEvidenceV1, derive_admitted_authorities_v1, derive_admitted_evidence_v1,
    },
    artifacts::{ArtifactSetV1, DerivedArtifactsV1, DerivedRecordV1, derive_artifact_facts},
    frame::{BuiltAccountV1, LogicalFrameV1, data_account, external, pack_frame, program, vacant},
    profile_ops,
    registers::{
        AdmittedCandidateProjectorV1, ContentProjectionKeysV1, EngineInputV1, EngineOutputV1,
        ObservedAccountV1, SpanWidthInputV1, derive_dynamic_span_geometry, run_engine,
        run_engine_with_admitted_candidate,
    },
    routes::{DerivedAuthorityV1, derive_authority},
};
use dclutch_execution_strategy_contract::{
    admitted_v3::{AdmittedInvocationContextV3, admitted_invocation_context_digest_v3},
    encode_register_bank_into,
    shadow_digest_v3::{
        ShadowRuntimeObservationV3, family_request_digest_v3, runtime_observations_digest_v3,
    },
    v2::{
        ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2, AuthenticatedScratchPageV2,
        SCRATCH_PAGE_HEADER_BYTES_V2, ScratchPageKindV2,
    },
};
use dclutch_release_set_contract::ArtifactReleaseIdV1;

/// Fixed-frame corpus: the Market, the root, the four Product content records,
/// and the external deployment identities the fixed Hot frame restates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixedCorpusV1 {
    /// Canonical Core Market account (key plus exact CoreState bytes).
    pub market: BuiltAccountV1,
    /// Canonical mutable family root account (Trading-owned).
    pub root: BuiltAccountV1,
    /// Finalized Product record.
    pub product: DerivedRecordV1,
    /// Finalized ResultDomain record.
    pub result_domain: DerivedRecordV1,
    /// Finalized Portfolio record.
    pub portfolio: DerivedRecordV1,
    /// Finalized linked-basis record.
    pub linked_basis: DerivedRecordV1,
    /// Core ProgramData account.
    pub core_programdata: Pubkey,
    /// Trading ProgramData account.
    pub trading_programdata: Pubkey,
}

/// The scenario: everything the artifacts do not determine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioV1<'a> {
    /// Family request bytes (after the Hot envelope).
    pub family_request: &'a [u8],
    /// Product-authenticated runtime item count.
    pub tail_count: u32,
    /// Trusted current slot.
    pub clock_slot: u64,
    /// Capability generation.
    pub generation: u64,
    /// Ed25519 evidence instruction data for Signed request profiles. Only
    /// offsets and public keys are read at build time; signatures may be zero.
    pub ed25519_evidence: Option<&'a [u8]>,
    /// Top-level index of the Hot-carrying instruction (1 on the canonical
    /// continuation).
    pub native_message_instruction_index: u16,
    /// Additional keys the enclosing ProgramTest installs itself (for example
    /// a token program); waist programs, sysvars, System and the payer are
    /// always treated as external.
    pub externally_installed_extra: &'a [Pubkey],
    /// The transaction payer's key (bound in the runtime frame by the
    /// campaign; named here so it is never rollback-snapshotted).
    pub payer: Pubkey,
}

/// Everything the builder constructed and derived.
#[derive(Clone, Debug)]
pub struct BuiltBundleV1 {
    /// Trading Hot instruction before Registry wrapping.
    pub hot_instruction: Instruction,
    /// All accounts, including externally installed identities.
    pub accounts: Vec<InstallAccountV1>,
    /// Keys the enclosing ProgramTest owns.
    pub externally_installed_keys: Vec<Pubkey>,
    /// Keys late-child refusal must preserve byte-for-byte.
    pub rollback_snapshot_keys: Vec<Pubkey>,
    /// The artifact-set derivations (records, action, seal).
    pub artifacts: DerivedArtifactsV1,
    /// The derived caller authorities, in walk order.
    pub authorities: Vec<DerivedAuthorityV1>,
    /// The final logical frame after adoption.
    pub logical: LogicalFrameV1,
    /// The engine's final run (registers, request bank, invocations).
    pub engine: EngineOutputV1,
    /// Authenticated dynamic fixed-span widths this bundle was packed at, one
    /// per span the AccountProfile declares (empty when it declares none).
    pub span_counts: Vec<u32>,
    /// Sole AccountProfile-owned input-scratch span, when the selected
    /// admitted strategy transports its input bank through runtime pages.
    pub transport_span: Option<u16>,
}

/// A complete admitted-AOT bundle and the derived request transcript that
/// explains every inserted caller-authority account.
#[derive(Clone, Debug)]
pub struct BuiltAdmittedBundleV1 {
    /// The ordinary bundle with strategy evidence and authorities inserted
    /// between the fixed frame and runtime suffix.
    pub bundle: BuiltBundleV1,
    /// Authenticated Certificate/Admission/ArtifactRelease/deployment chain.
    pub evidence: DerivedAdmittedEvidenceV1,
    /// Complete context whose digest every accelerator request carries.
    pub invocation_context: AdmittedInvocationContextV3,
    /// Exact context digest.
    pub invocation_context_digest: dclutch_core_contract::ContentId,
    /// Complete input bank and one exact request/PDA per canonical chunk.
    pub admitted_authorities: DerivedAdmittedAuthoritiesV1,
}

/// One account with its derived rollback classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallAccountV1 {
    /// Exact account identity.
    pub key: Pubkey,
    /// Exact initial account state.
    pub account: Account,
    /// Whether late-child refusal must preserve this account byte-for-byte.
    pub snapshot_for_rollback: bool,
}

/// Everything `build_bundle` consumes.
pub struct BundleInputV1<'a> {
    /// The ten emitted artifact byte strings.
    pub set: ArtifactSetV1<'a>,
    /// Release-waist facts.
    pub waist: WaistFactsV1,
    /// The scenario corpus.
    pub scenario: ScenarioV1<'a>,
    /// The fixed-frame corpus.
    pub fixed: FixedCorpusV1,
    /// Runtime-frame corpus bindings at self coordinates. Coordinates the
    /// engine derives (lifecycle-created states, caller authorities) and
    /// coordinates the profile aliases must not appear here.
    pub bindings: &'a [(usize, BuiltAccountV1)],
    /// Current rent schedule.
    pub rent: &'a Rent,
}

fn digest32(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Mine the bumps this bundle's readers would otherwise search for on chain.
///
/// # Why a campaign builder owes this at all
///
/// `HotBumpHintsV1` has been read by Trading, the Dealer accelerator and
/// Custody since it was added, and `dclutch-operator`'s own producers --
/// `direct_inline_v3` and `dealer_lp_hot_v4` -- have filled it since. THIS
/// builder never did, so every packet the campaign has ever emitted carried the
/// all-zero block and every reader on the route searched. That is not a neutral
/// default: it is the pre-hint route, and it is the one the campaign's compute
/// figures were being read off. `live_claims_graph` was the same defect one
/// account over, and it was repaired the same way -- by recording what a real
/// producer would have written.
///
/// Each search removed is a `find_program_address` walking down from 255 at
/// 1,500 CU per rejected candidate, on a depth drawn from the fixture keys, so
/// what it takes out of the measurement is DRAW: the worst case, not the mean.
///
/// # Which slots are filled, and which are deliberately left searching
///
/// * `market` and `root` come out of the fixed corpus this builder already
///   binds and validates, so both are exact.
/// * `child_relay[1]` is Custody's transfer authority, whose seeds are the
///   Market and the release set -- the same pair for every Custody leg of every
///   family, which is what makes ONE slot correct for a route with two of them.
/// * `child_relay[0]` is Custody's own replay cursor, whose seeds end in the
///   projected child request's replay CONTEXT. This builder is handed the
///   family request and does not project the children, so the slot stays zero
///   and Custody searches -- exactly as `dealer_lp_hot_v4` leaves it, and for
///   the same reason.
/// * `child_caller` cannot be mined at all: those seeds end in a digest over a
///   request projected on chain, so nothing off chain holds their preimage.
/// * `lifecycle` is the family's created accounts in materialization order and
///   is likewise not projected here.
///
/// A slot left zero is correct and merely slower; that is the whole contract of
/// the block. No conjunct moves either way: every hint is fed to a
/// `create_program_address` whose result is compared with the account the frame
/// supplies, so a wrong byte names a different address and refuses at an
/// equality that was always there.
///
/// # This function is TOTAL, and that is a correction
///
/// It was fallible in all three of its reads, and every one of them refused the
/// WHOLE BUNDLE on a corpus it could not decode. That inverts the block's
/// contract: `HotBumpHintsV1`'s own doc says an unset hint is zero and the
/// reader searches, so a producer that cannot mine a slot owes a zero, not a
/// refusal -- exactly as `derive_hinted` is total in all three programs that
/// consume these bytes.
///
/// The cost of the inversion was measured on 2026-09-03.
/// `series_pre_market_expiry_program_test` stages a Series Expire that lands
/// BEFORE the future Market exists, so `fixed.market` is not a `CoreState`, and
/// `82465e00b`'s `CoreState::decode` turned that into `Binding(239)` on all
/// three rows -- a refusal wearing the shape of an unrelated defect. It masked
/// the real one, which is still there: `Projection("borrowed-range-resolve")`.
/// A bundle that would merely have been slower was reported as unbuildable.
fn mine_bump_hints_v1(input: &BundleInputV1<'_>) -> HotBumpHintsV1 {
    let market_bump = CoreState::decode(&input.fixed.market.account.data)
        .ok()
        .map(|market| {
            Pubkey::find_program_address(
                &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
                &input.waist.core_program,
            )
            .1
        })
        .unwrap_or_default();
    let root_bump = input
        .fixed
        .root
        .account
        .data
        .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
        .and_then(|header| CapabilityRootHeaderV1::decode(header).ok())
        .map(|root| {
            Pubkey::find_program_address(&root.seeds().as_slices(), &input.waist.trading_program).1
        })
        .unwrap_or_default();
    // Always derivable: its seeds are the Market ADDRESS and the release set,
    // neither of which is read out of an account body.
    let transfer_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(input.fixed.market.key.to_bytes(), input.waist.release_set)
            .as_slices(),
        &input.waist.custody_program,
    )
    .1;
    HotBumpHintsV1 {
        market: market_bump,
        root: root_bump,
        child_relay: [0, transfer_authority],
        ..HotBumpHintsV1::ABSENT
    }
}

fn decode_execution_account_profile<'a>(
    schema: [u8; 32],
    bytes: &'a [u8],
) -> Result<AccountProfileV2<'a>, BuilderError> {
    match schema {
        ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2 => {
            AccountProfileV2::decode(bytes).map_err(|_| BuilderError::Artifact)
        }
        ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V3 => AccountProfileV3::decode(bytes)
            .map(AccountProfileV3::base)
            .map_err(|_| BuilderError::Artifact),
        _ => Err(BuilderError::Artifact),
    }
}

fn placeholder_key(coordinate: usize) -> Pubkey {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch-bundle-builder/placeholder/v1");
    hasher.update((coordinate as u64).to_le_bytes());
    Pubkey::new_from_array(hasher.finalize().into())
}

/// Build one complete family Hot bundle.
///
/// The adoption loop runs the host engine, adopts every derived coordinate
/// (lifecycle-created states, caller authorities), and re-runs until the
/// derivations are the observed keys. Two rounds settle every known family;
/// four is the refusal bound.
/// The tail width the EXECUTOR will carve this frame at, which is not always
/// the product's outcome count.
///
/// `hot_v3::project_tail_count` returns `None` for an AccountProfile that
/// declares no `OP_PROJECT_TAIL_COUNT_U32`, and its caller reads that absence as
/// width zero -- a fixed topology has no per-outcome tail to project. This host
/// read the SCENARIO's outcome count instead, so for every such profile the two
/// sides put a different number in `AdmittedInvocationContextV3::tail_count`,
/// and that field is inside the digest every admitted caller-authority PDA is
/// derived from.
///
/// MEASURED on real ELFs 2026-09-02, the accepted campaign's equity Add: chain
/// `tail_count` 0 against host 3, every other context field equal to the byte --
/// including the runtime-observations digest -- so the context digests differed,
/// the host's authority for chunk 0 was not the one Trading derived, and
/// `admitted_composition_v3.rs` refused `TradingSbfError::Release` 0x4001 at
/// 763,845 CU with the accelerator never invoked. The LP Open, whose profile
/// DOES project a tail, agreed on both sides and executed. Two authors for one
/// number, and only one of them is the executor.
fn projected_tail_count_v1(
    profile: AccountProfileV2<'_>,
    product_tail_count: u32,
) -> Result<u32, BuilderError> {
    if profile
        .tail_count_projection()
        .map_err(|_| BuilderError::Artifact)?
        .is_none()
    {
        Ok(0)
    } else {
        Ok(product_tail_count)
    }
}

pub fn build_bundle(input: &BundleInputV1<'_>) -> Result<BuiltBundleV1, BuilderError> {
    build_bundle_with_admitted_candidate(input, None)
}

fn build_bundle_with_admitted_candidate(
    input: &BundleInputV1<'_>,
    candidate_projector: Option<&AdmittedCandidateProjectorV1<'_>>,
) -> Result<BuiltBundleV1, BuilderError> {
    let facts = derive_artifact_facts(input.set, input.waist, input.scenario.family_request)?;
    let profile =
        decode_execution_account_profile(facts.account_profile.schema, input.set.account_profile)?;
    let tail_count = projected_tail_count_v1(profile, input.scenario.tail_count)?;
    // The span widths come before the frame exists: they *are* the frame's
    // width. Derived from the artifacts and the family request, exactly as the
    // Hot executor derives them before it expands one account.
    let span_geometry = derive_dynamic_span_geometry(&SpanWidthInputV1 {
        profile,
        request_profile_bytes: input.set.request_profile,
        request_profile_schema: facts.request_profile.schema,
        effect_bytes: input.set.effect,
        effect_schema: facts.effect.schema,
        strategy_bytes: input.set.strategy,
        waist: input.waist,
        tail_count,
        family_request: input.scenario.family_request,
        clock_slot: input.scenario.clock_slot,
    })?;
    let span_counts = span_geometry.widths;
    let transport_span = span_geometry.transport_span;
    let spans = span_counts.as_slice();
    let logical_count = profile_ops::logical_count(profile, tail_count, spans)?;

    // The Hot envelope: root prestate is the digest of the bound root bytes.
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(input.scenario.family_request.len()).map_err(|_| BuilderError::Arithmetic)?,
        input.waist.release_set,
        input.fixed.market.key.to_bytes(),
        input.scenario.generation,
        digest32(&input.fixed.root.account.data),
    )
    .map_err(|_| BuilderError::Binding(line!()))?
    .with_bump_hints(mine_bump_hints_v1(input));
    let mut instruction_data =
        Vec::with_capacity(HOT_EXECUTION_ENVELOPE_BYTES_V3 + input.scenario.family_request.len());
    instruction_data.extend_from_slice(&envelope.to_bytes());
    instruction_data.extend_from_slice(input.scenario.family_request);

    // Seed the logical frame: campaign bindings plus the shared runtime prefix.
    let mut frame = LogicalFrameV1::new(logical_count);
    frame.bind(0, input.fixed.root.clone())?;
    frame.bind(1, finalized_raw(input.rent, &facts.config))?;
    frame.bind(2, finalized_raw(input.rent, &input.fixed.product))?;
    frame.bind(3, finalized_raw(input.rent, &input.fixed.portfolio))?;
    frame.bind(4, finalized_raw(input.rent, &input.fixed.linked_basis))?;
    for (coordinate, value) in input.bindings {
        frame.bind(*coordinate, value.clone())?;
    }
    // Placeholders for engine-derived coordinates, so pass one can observe a
    // complete frame. Alias coordinates stay unbound.
    let mut derived_coordinates: Vec<usize> = Vec::new();
    for coordinate in 0..logical_count {
        let representative = profile_ops::representative(profile, tail_count, spans, coordinate)?;
        if representative == coordinate && frame.get(coordinate).is_none() {
            derived_coordinates.push(coordinate);
            frame.adopt(coordinate, vacant(placeholder_key(coordinate)))?;
        }
    }

    let content_keys = ContentProjectionKeysV1 {
        selected_config: facts.config.digest,
        product_root: input.fixed.product.digest,
        portfolio: input.fixed.portfolio.digest,
        linked_basis: input.fixed.linked_basis.digest,
    };

    // The adoption loop.
    let mut engine_output = None;
    let mut authorities = Vec::new();
    for _round in 0..4 {
        let observations = observe_frame(profile, tail_count, spans, &frame)?;
        for (coordinate, observation) in observations.iter().enumerate() {
            let ordinal = profile_ops::ordinal(profile, tail_count, spans, coordinate)?;
            let geometry = profile_ops::geometry(profile, tail_count, spans, ordinal)?;
            let declared = geometry.data();
            let observed = observation.data.len();
            let mismatch = match declared {
                dclutch_account_profile_contract::v2::PhysicalAccountDataGeometryV2::Exact {
                    bytes,
                } => observed != bytes,
                dclutch_account_profile_contract::v2::PhysicalAccountDataGeometryV2::VacantOrExact {
                    live_bytes,
                } => observed != 0 && observed != live_bytes,
                dclutch_account_profile_contract::v2::PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable {
                    minimum_bytes,
                } => observed < minimum_bytes,
                dclutch_account_profile_contract::v2::PhysicalAccountDataGeometryV2::Opaque => false,
            };
            if mismatch {
                std::eprintln!(
                    "width probe: coordinate {coordinate} observed {observed} declared {declared:?}"
                );
            }
        }
        let engine_input = EngineInputV1 {
            profile,
            request_profile_bytes: input.set.request_profile,
            request_profile_schema: facts.request_profile.schema,
            lifecycle_bytes: input.set.lifecycle,
            transition_bytes: input.set.transition,
            effect_bytes: input.set.effect,
            effect_schema: facts.effect.schema,
            action: facts.action,
            waist: input.waist,
            tail_count,
            family_request: input.scenario.family_request,
            instruction_data: &instruction_data,
            ed25519_evidence: input.scenario.ed25519_evidence,
            native_message_instruction_index: input.scenario.native_message_instruction_index,
            clock_slot: input.scenario.clock_slot,
            market: input.fixed.market.key.to_bytes(),
            generation: input.scenario.generation,
            observations: &observations,
            content_keys,
            span_counts: spans,
            rent: input.rent,
        };
        let output = if candidate_projector.is_some() {
            run_engine_with_admitted_candidate(&engine_input, candidate_projector)?
        } else {
            run_engine(&engine_input)?
        };
        let mut changed = false;
        for state in &output.lifecycle_states {
            let current = frame
                .get(state.coordinate)
                .ok_or(BuilderError::Binding(line!()))?
                .key;
            if current != state.derived {
                if !derived_coordinates.contains(&state.coordinate) {
                    // The campaign bound this coordinate to a key the policy
                    // derives differently: a corpus contradiction, not ours to
                    // overwrite.
                    return Err(BuilderError::Binding(line!()));
                }
                frame.adopt(state.coordinate, vacant(state.derived))?;
                changed = true;
            }
        }
        authorities.clear();
        for invocation in &output.invocations {
            if let Some(authority) = derive_authority(
                invocation,
                input.waist.release_set,
                input.waist.trading_program,
            )? {
                let current = frame
                    .get(authority.coordinate)
                    .ok_or(BuilderError::Binding(line!()))?
                    .key;
                if current != authority.authority {
                    if !derived_coordinates.contains(&authority.coordinate) {
                        return Err(BuilderError::Binding(line!()));
                    }
                    frame.adopt(authority.coordinate, vacant(authority.authority))?;
                    changed = true;
                }
                authorities.push(authority);
            }
        }
        let complete = output.complete;
        engine_output = Some(output);
        if !changed && complete {
            break;
        }
        engine_output = None;
    }
    let engine = engine_output.ok_or(BuilderError::Projection("adoption-divergence"))?;

    // Pack the runtime frame; assemble the fixed frame; join them.
    let packed = pack_frame(profile, tail_count, spans, &frame)?;
    let fixed = fixed_hot_frame(input, &facts)?;

    let mut metas: Vec<AccountMeta> = fixed.iter().map(|value| value.meta.clone()).collect();
    metas.extend(
        packed
            .iter()
            .skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
            .map(|value| value.meta.clone()),
    );
    let hot_instruction = Instruction {
        program_id: input.waist.trading_program,
        accounts: metas,
        data: instruction_data,
    };

    let mut accounts: Vec<InstallAccountV1> = fixed
        .into_iter()
        .map(|value| InstallAccountV1 {
            key: value.built.key,
            account: value.built.account,
            snapshot_for_rollback: value.snapshot,
        })
        .collect();
    for candidate in packed.iter().skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3) {
        if !accounts
            .iter()
            .any(|value| value.key == candidate.built.key)
        {
            let snapshot = candidate.snapshot && candidate.built.key != input.scenario.payer;
            accounts.push(InstallAccountV1 {
                key: candidate.built.key,
                account: candidate.built.account.clone(),
                snapshot_for_rollback: snapshot,
            });
        }
    }
    let rollback_snapshot_keys = accounts
        .iter()
        .filter(|value| value.snapshot_for_rollback)
        .map(|value| value.key)
        .collect();
    let external_candidates = [
        input.waist.activation_cache,
        input.waist.registry_program,
        input.waist.trading_program,
        input.waist.core_program,
        input.waist.claims_program,
        input.waist.custody_program,
        input.fixed.trading_programdata,
        input.fixed.core_programdata,
        input.scenario.payer,
        system_program::ID,
        sysvar::rent::ID,
        sysvar::instructions::ID,
    ];
    let externally_installed_keys = external_candidates
        .into_iter()
        .chain(input.scenario.externally_installed_extra.iter().copied())
        .filter(|candidate| accounts.iter().any(|value| value.key == *candidate))
        .collect();

    Ok(BuiltBundleV1 {
        hot_instruction,
        accounts,
        externally_installed_keys,
        rollback_snapshot_keys,
        artifacts: facts,
        authorities,
        logical: frame,
        engine,
        span_counts,
        transport_span,
    })
}

/// Build the ordinary artifact-derived bundle and insert the complete
/// admitted-AOT suffix.
///
/// [`build_bundle`] remains the byte-for-byte owner of the interpreted/legacy
/// shape. This function only adds accounts after its fixed 39-account prefix,
/// so callers that do not supply admitted evidence receive exactly their
/// previous instruction and account bytes.
pub fn build_admitted_bundle(
    input: &BundleInputV1<'_>,
    admitted: AdmittedAotInputV1<'_>,
) -> Result<BuiltAdmittedBundleV1, BuilderError> {
    let bundle = build_bundle(input)?;
    finish_admitted_bundle(input, admitted, bundle)
}

/// Build an admitted-AOT bundle whose authenticated accelerator, rather than
/// the interpreted Transition artifact, owns the candidate register bank.
///
/// The projector receives a private copy of the exact post-lifecycle input
/// bank. It can only affect Effect projection and child-authority derivation;
/// admitted request chunks, their input-bank digest, and the runtime
/// observation transcript remain derived from the unmodified preplan bank.
pub fn build_admitted_bundle_with_candidate_v1(
    input: &BundleInputV1<'_>,
    admitted: AdmittedAotInputV1<'_>,
    candidate_projector: &AdmittedCandidateProjectorV1<'_>,
) -> Result<BuiltAdmittedBundleV1, BuilderError> {
    let bundle = build_bundle_with_admitted_candidate(input, Some(candidate_projector))?;
    finish_admitted_bundle(input, admitted, bundle)
}

fn finish_admitted_bundle(
    input: &BundleInputV1<'_>,
    admitted: AdmittedAotInputV1<'_>,
    mut bundle: BuiltBundleV1,
) -> Result<BuiltAdmittedBundleV1, BuilderError> {
    let evidence = derive_admitted_evidence_v1(input.waist.registry_program, input.set, admitted)?;
    let descriptor =
        CapabilityProgramV4::decode(input.set.descriptor).map_err(|_| BuilderError::Artifact)?;
    let profile = decode_execution_account_profile(
        bundle.artifacts.account_profile.schema,
        input.set.account_profile,
    )?;
    prepare_admitted_transport_pages(&mut bundle, input, profile)?;
    let tail_count = projected_tail_count_v1(profile, input.scenario.tail_count)?;
    let runtime_observations_digest = admitted_runtime_observations_digest(
        profile,
        tail_count,
        &bundle.span_counts,
        bundle.transport_span,
        &bundle.logical,
        ContentProjectionKeysV1 {
            selected_config: bundle.artifacts.config.digest,
            product_root: input.fixed.product.digest,
            portfolio: input.fixed.portfolio.digest,
            linked_basis: input.fixed.linked_basis.digest,
        },
    )?;
    let artifact_release = ArtifactReleaseIdV1::new(evidence.artifact_release.digest)
        .map_err(|_| BuilderError::Artifact)?;
    let invocation_context = AdmittedInvocationContextV3 {
        release_set: content(input.waist.release_set)?,
        market: content(input.fixed.market.key.to_bytes())?,
        root: content(input.fixed.root.key.to_bytes())?,
        registry_program: content(input.waist.registry_program.to_bytes())?,
        trading_program: content(input.waist.trading_program.to_bytes())?,
        accelerator_program: content(evidence.accelerator_program.key.to_bytes())?,
        capability_program: content(bundle.artifacts.descriptor.digest)?,
        account_profile: descriptor.account_profile().program(),
        request_profile: descriptor.request_profile().program(),
        transition: descriptor.transition().program(),
        effect: descriptor.effect().program(),
        lifecycle: descriptor.derivation_policy(),
        strategy: content(bundle.artifacts.strategy.digest)?,
        certificate: content(evidence.certificate.digest)?,
        admission: content(evidence.admission.digest)?,
        artifact_release,
        config: content(bundle.artifacts.config.digest)?,
        product: content(input.fixed.product.digest)?,
        portfolio: content(input.fixed.portfolio.digest)?,
        linked_basis: content(input.fixed.linked_basis.digest)?,
        family_request_digest: family_request_digest_v3(input.scenario.family_request)
            .map_err(|_| BuilderError::Artifact)?,
        runtime_observations_digest,
        root_prestate_digest: content(digest32(&input.fixed.root.account.data))?,
        selected_action: bundle.artifacts.action,
        tail_count,
        account_count: u32::try_from(bundle.logical.len()).map_err(|_| BuilderError::Arithmetic)?,
        scalar_count: u32::try_from(bundle.engine.input_scalars.len())
            .map_err(|_| BuilderError::Arithmetic)?,
        identity_count: u32::try_from(bundle.engine.input_identities.len())
            .map_err(|_| BuilderError::Arithmetic)?,
    };
    let invocation_context_digest = admitted_invocation_context_digest_v3(invocation_context)
        .map_err(|_| BuilderError::Artifact)?;
    let admitted_authorities = derive_admitted_authorities_v1(AdmittedAuthorityInputV1 {
        trading_program: input.waist.trading_program,
        release_set: content(input.waist.release_set)?,
        market: input.fixed.market.key,
        root: input.fixed.root.key,
        strategy_program: content(bundle.artifacts.strategy.digest)?,
        certificate_program: content(evidence.certificate.digest)?,
        capability_program: content(bundle.artifacts.descriptor.digest)?,
        invocation_context: invocation_context_digest,
        family_request_digest: invocation_context.family_request_digest,
        transport: if bundle.transport_span.is_some() {
            dclutch_execution_strategy_contract::v2::RequestTransportV2::ScratchPages
        } else {
            dclutch_execution_strategy_contract::v2::RequestTransportV2::Inline
        },
        // Read out of the Strategy record this bundle installs, never chosen
        // here: the record is the only authority for which transport a route
        // is on, and a host that picked its own would build a frame no
        // deployed Strategy asks for.
        profile: dclutch_execution_strategy_contract::v2::ExecutionStrategyProgramV2::decode(
            &bundle.artifacts.strategy.bytes,
        )
        .and_then(
            dclutch_execution_strategy_contract::v2::ExecutionStrategyProgramV2::transport_profile,
        )
        .map_err(|_| BuilderError::Artifact)?,
        accelerator_program: evidence.accelerator_program.key,
        tail_count,
        scalars: &bundle.engine.input_scalars,
        identities: &bundle.engine.input_identities,
    })?;
    materialize_admitted_transport_pages(
        &mut bundle,
        input,
        profile,
        invocation_context_digest,
        &admitted_authorities.input_bank,
    )?;
    insert_admitted_suffix(&mut bundle, input, &evidence, &admitted_authorities)?;
    Ok(BuiltAdmittedBundleV1 {
        bundle,
        evidence,
        invocation_context,
        invocation_context_digest,
        admitted_authorities,
    })
}

fn content(bytes: [u8; 32]) -> Result<dclutch_core_contract::ContentId, BuilderError> {
    dclutch_core_contract::ContentId::new(bytes).map_err(|_| BuilderError::Artifact)
}

fn admitted_runtime_observations_digest(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    spans: &[u32],
    transport_span: Option<u16>,
    frame: &LogicalFrameV1,
    keys: ContentProjectionKeysV1,
) -> Result<dclutch_core_contract::ContentId, BuilderError> {
    let observations = observe_frame(profile, tail_count, spans, frame)?;
    let transport = transport_span_range(profile, spans, transport_span)?;
    let shadow = observations
        .iter()
        .enumerate()
        .map(|(coordinate, observed)| {
            let representative =
                profile_ops::representative(profile, tail_count, spans, coordinate)?;
            let key = match representative {
                1 => keys.selected_config,
                2 => keys.product_root,
                3 => keys.portfolio,
                4 => keys.linked_basis,
                _ => observed.key,
            };
            // Authenticated input pages embed this invocation-context digest.
            // Hashing their exact bytes here would be a cryptographic
            // self-reference. Their keys and account facts remain in this
            // transcript; exact page bytes are independently committed by the
            // request's input-bank digest and page decoder.
            // Loader state is identity, not prestate: the on-chain transcript
            // omits an executable or upgradeable-loader-owned coordinate's
            // bytes, so the host precompute omits exactly the same ones.
            let loader_state = observed.executable
                || observed.owner == solana_sdk_ids::bpf_loader_upgradeable::ID.to_bytes();
            let data = if loader_state
                || transport
                    .as_ref()
                    .is_some_and(|range| range.contains(&coordinate))
            {
                [].as_slice()
            } else {
                observed.data.as_slice()
            };
            Ok(ShadowRuntimeObservationV3 {
                key,
                owner: observed.owner,
                lamports: observed.lamports,
                data,
                signer: false,
                writable: false,
                executable: observed.executable,
            })
        })
        .collect::<Result<Vec<_>, BuilderError>>()?;
    runtime_observations_digest_v3(&shadow).map_err(|_| BuilderError::Artifact)
}

fn transport_span_range(
    profile: AccountProfileV2<'_>,
    spans: &[u32],
    transport_span: Option<u16>,
) -> Result<Option<core::ops::Range<usize>>, BuilderError> {
    let Some(transport_span) = transport_span else {
        return Ok(None);
    };
    if !profile.uses_dynamic_fixed_spans()
        || spans.len() != usize::from(profile.dynamic_fixed_span_count())
        || transport_span >= profile.dynamic_fixed_span_count()
    {
        return Err(BuilderError::Spans("transport-range"));
    }
    let span = profile
        .dynamic_fixed_span(transport_span)
        .map_err(|_| BuilderError::Spans("transport-range"))?;
    let prior = spans
        .get(..usize::from(transport_span))
        .ok_or(BuilderError::Spans("transport-range"))?
        .iter()
        .try_fold(0_usize, |sum, width| {
            sum.checked_add(usize::try_from(*width).map_err(|_| BuilderError::Arithmetic)?)
                .ok_or(BuilderError::Arithmetic)
        })?;
    let start = usize::from(span.insertion_coordinate())
        .checked_add(prior)
        .ok_or(BuilderError::Arithmetic)?;
    let width = usize::try_from(
        *spans
            .get(usize::from(transport_span))
            .ok_or(BuilderError::Spans("transport-range"))?,
    )
    .map_err(|_| BuilderError::Arithmetic)?;
    Ok(Some(
        start..start.checked_add(width).ok_or(BuilderError::Arithmetic)?,
    ))
}

fn prepare_admitted_transport_pages(
    bundle: &mut BuiltBundleV1,
    input: &BundleInputV1<'_>,
    profile: AccountProfileV2<'_>,
) -> Result<(), BuilderError> {
    let Some(range) = transport_span_range(profile, &bundle.span_counts, bundle.transport_span)?
    else {
        return Ok(());
    };
    let bank_len = bundle
        .engine
        .input_scalars
        .len()
        .checked_mul(8)
        .and_then(|value| {
            bundle
                .engine
                .input_identities
                .len()
                .checked_mul(32)
                .and_then(|identities| value.checked_add(identities))
        })
        .ok_or(BuilderError::Arithmetic)?;
    let mut bank = vec![0_u8; bank_len];
    encode_register_bank_into(
        &bundle.engine.input_scalars,
        &bundle.engine.input_identities,
        &mut bank,
    )
    .map_err(|_| BuilderError::Artifact)?;
    let bank_digest = digest32(&bank);
    for (page_index, coordinate) in range.enumerate() {
        let payload_start = page_index
            .checked_mul(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2)
            .ok_or(BuilderError::Arithmetic)?;
        let payload_end = payload_start
            .checked_add(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2)
            .map(|end| end.min(bank.len()))
            .ok_or(BuilderError::Arithmetic)?;
        if payload_start >= payload_end {
            return Err(BuilderError::Spans("transport-pages"));
        }
        let data_len = SCRATCH_PAGE_HEADER_BYTES_V2
            .checked_add(payload_end - payload_start)
            .ok_or(BuilderError::Arithmetic)?;
        let mut key = Sha256::new();
        key.update(b"dclutch:bundle-builder:input-scratch-page:v1");
        key.update(input.waist.release_set);
        key.update(input.fixed.market.key.as_ref());
        key.update(input.fixed.root.key.as_ref());
        key.update(bundle.artifacts.strategy.digest);
        key.update(bank_digest);
        key.update(
            u32::try_from(page_index)
                .map_err(|_| BuilderError::Arithmetic)?
                .to_le_bytes(),
        );
        let page = data_account(
            input.rent,
            Pubkey::new_from_array(key.finalize().into()),
            input.waist.trading_program,
            vec![0_u8; data_len],
        );
        replace_runtime_account(bundle, coordinate, page)?;
    }
    Ok(())
}

fn materialize_admitted_transport_pages(
    bundle: &mut BuiltBundleV1,
    input: &BundleInputV1<'_>,
    profile: AccountProfileV2<'_>,
    invocation_context: dclutch_core_contract::ContentId,
    input_bank: &[u8],
) -> Result<(), BuilderError> {
    let Some(range) = transport_span_range(profile, &bundle.span_counts, bundle.transport_span)?
    else {
        return Ok(());
    };
    let scalar_count =
        u32::try_from(bundle.engine.input_scalars.len()).map_err(|_| BuilderError::Arithmetic)?;
    let identity_count = u32::try_from(bundle.engine.input_identities.len())
        .map_err(|_| BuilderError::Arithmetic)?;
    let bank_digest = content(digest32(input_bank))?;
    let strategy = content(bundle.artifacts.strategy.digest)?;
    for (page_index, coordinate) in range.enumerate() {
        let payload_start = page_index
            .checked_mul(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2)
            .ok_or(BuilderError::Arithmetic)?;
        let payload_end = payload_start
            .checked_add(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2)
            .map(|end| end.min(input_bank.len()))
            .ok_or(BuilderError::Arithmetic)?;
        let payload = input_bank
            .get(payload_start..payload_end)
            .ok_or(BuilderError::Arithmetic)?;
        let current = bundle
            .logical
            .get(coordinate)
            .ok_or(BuilderError::Binding(line!()))?
            .clone();
        let page = AuthenticatedScratchPageV2::new(
            ScratchPageKindV2::Input,
            content(input.waist.trading_program.to_bytes())?,
            strategy,
            invocation_context,
            bank_digest,
            projected_tail_count_v1(profile, input.scenario.tail_count)?,
            scalar_count,
            identity_count,
            u32::try_from(page_index).map_err(|_| BuilderError::Arithmetic)?,
            payload,
        )
        .map_err(|_| BuilderError::Artifact)?;
        let mut data = vec![0_u8; SCRATCH_PAGE_HEADER_BYTES_V2 + payload.len()];
        page.encode_into(&mut data)
            .map_err(|_| BuilderError::Artifact)?;
        let materialized = data_account(input.rent, current.key, input.waist.trading_program, data);
        if materialized.account.lamports != current.account.lamports {
            return Err(BuilderError::Binding(line!()));
        }
        replace_runtime_account(bundle, coordinate, materialized)?;
    }
    Ok(())
}

fn replace_runtime_account(
    bundle: &mut BuiltBundleV1,
    coordinate: usize,
    replacement: BuiltAccountV1,
) -> Result<(), BuilderError> {
    let old = bundle
        .logical
        .get(coordinate)
        .ok_or(BuilderError::Binding(line!()))?
        .key;
    let matching_metas = bundle
        .hot_instruction
        .accounts
        .iter_mut()
        .filter(|meta| meta.pubkey == old)
        .collect::<Vec<_>>();
    if matching_metas.len() != 1 {
        return Err(BuilderError::Binding(line!()));
    }
    matching_metas
        .into_iter()
        .next()
        .ok_or(BuilderError::Binding(line!()))?
        .pubkey = replacement.key;
    let install = bundle
        .accounts
        .iter_mut()
        .find(|account| account.key == old)
        .ok_or(BuilderError::Binding(line!()))?;
    install.key = replacement.key;
    install.account = replacement.account.clone();
    bundle.logical.adopt(coordinate, replacement)
}

fn insert_admitted_suffix(
    bundle: &mut BuiltBundleV1,
    input: &BundleInputV1<'_>,
    evidence: &DerivedAdmittedEvidenceV1,
    authorities: &DerivedAdmittedAuthoritiesV1,
) -> Result<(), BuilderError> {
    let mut extras = vec![
        finalized_raw(input.rent, &evidence.certificate),
        vacant(evidence.certificate.staging),
        finalized_raw(input.rent, &evidence.admission),
        vacant(evidence.admission.staging),
        finalized_raw(input.rent, &evidence.artifact_release),
        vacant(evidence.artifact_release.staging),
        evidence.accelerator_program.clone(),
        evidence.accelerator_programdata.clone(),
    ];
    extras.extend(
        authorities
            .entries
            .iter()
            .map(|entry| vacant(entry.authority)),
    );
    // The output page, if this transport has one, and it is the ONLY writable
    // account this suffix carries. It is provisioned here the way a client
    // provisions one on a live chain -- an account owned by the accelerator,
    // wide enough for the bank, created once and reused -- and its data is
    // left as whatever the last transaction wrote, because a page that is not
    // zeroed between runs is exactly the case the digest has to bind.
    let page_index = extras.len();
    if let Some(page) = authorities.output_page {
        extras.push(BuiltAccountV1 {
            key: page,
            account: Account {
                lamports: input
                    .rent
                    .minimum_balance(authorities.input_bank.len())
                    .max(1),
                data: vec![0_u8; authorities.input_bank.len()],
                owner: evidence.accelerator_program.key,
                executable: false,
                rent_epoch: 0,
            },
            observed: None,
        });
    }
    let metas = extras
        .iter()
        .enumerate()
        .map(|(index, account)| {
            if authorities.output_page.is_some() && index == page_index {
                AccountMeta::new(account.key, false)
            } else {
                AccountMeta::new_readonly(account.key, false)
            }
        })
        .collect::<Vec<_>>();
    bundle.hot_instruction.accounts.splice(
        HOT_FIXED_ACCOUNT_COUNT_V3..HOT_FIXED_ACCOUNT_COUNT_V3,
        metas,
    );
    for built in extras {
        if !bundle
            .accounts
            .iter()
            .any(|account| account.key == built.key)
        {
            bundle.accounts.push(InstallAccountV1 {
                key: built.key,
                account: built.account,
                snapshot_for_rollback: false,
            });
        }
    }
    for key in [
        evidence.accelerator_program.key,
        evidence.accelerator_programdata.key,
    ] {
        if !bundle.externally_installed_keys.contains(&key) {
            bundle.externally_installed_keys.push(key);
        }
    }
    Ok(())
}

/// Observe the current frame with per-coordinate profile privileges.
fn observe_frame(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    spans: &[u32],
    frame: &LogicalFrameV1,
) -> Result<Vec<ObservedAccountV1>, BuilderError> {
    (0..frame.len())
        .map(|coordinate| {
            let built = frame.resolve(profile, tail_count, spans, coordinate)?;
            let ordinal = profile_ops::ordinal(profile, tail_count, spans, coordinate)?;
            let privileges =
                profile_ops::geometry(profile, tail_count, spans, ordinal)?.privileges();
            let view = built.chain_view();
            Ok(ObservedAccountV1 {
                key: built.key.to_bytes(),
                owner: view.owner.to_bytes(),
                lamports: view.lamports,
                data: view.data.clone(),
                signer: privileges.signer(),
                writable: privileges.writable(),
                executable: view.executable,
            })
        })
        .collect()
}

struct FixedAccountV1 {
    built: BuiltAccountV1,
    meta: AccountMeta,
    snapshot: bool,
}

fn fixed_readonly(built: BuiltAccountV1) -> FixedAccountV1 {
    let meta = AccountMeta::new_readonly(built.key, false);
    FixedAccountV1 {
        built,
        meta,
        snapshot: false,
    }
}

fn fixed_writable(built: BuiltAccountV1) -> FixedAccountV1 {
    let meta = AccountMeta::new(built.key, false);
    FixedAccountV1 {
        built,
        meta,
        snapshot: true,
    }
}

fn finalized_raw(rent: &Rent, record: &DerivedRecordV1) -> BuiltAccountV1 {
    data_account(rent, record.raw, record.owner, record.bytes.clone())
}

/// Assemble the 39-account fixed Hot frame from the derived records, the
/// fixed corpus, and the waist. The root is the sole writable coordinate.
fn fixed_hot_frame(
    input: &BundleInputV1<'_>,
    facts: &DerivedArtifactsV1,
) -> Result<Vec<FixedAccountV1>, BuilderError> {
    let rent = input.rent;
    let mut fixed: Vec<Option<FixedAccountV1>> = Vec::new();
    fixed.resize_with(HOT_FIXED_ACCOUNT_COUNT_V3, || None);
    let mut set = |index: usize, value: FixedAccountV1| -> Result<(), BuilderError> {
        *fixed.get_mut(index).ok_or(BuilderError::Profile(line!()))? = Some(value);
        Ok(())
    };
    set(
        HOT_MARKET_ACCOUNT_V3,
        fixed_readonly(input.fixed.market.clone()),
    )?;
    set(
        HOT_ROOT_ACCOUNT_V3,
        fixed_writable(input.fixed.root.clone()),
    )?;
    for (raw, staging, record) in [
        (
            HOT_MANIFEST_RAW_ACCOUNT_V3,
            HOT_MANIFEST_STAGING_ACCOUNT_V3,
            &facts.manifest,
        ),
        (
            HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
            HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
            &facts.program_set,
        ),
        (
            HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
            HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
            &facts.descriptor,
        ),
        (
            HOT_CONFIG_RAW_ACCOUNT_V3,
            HOT_CONFIG_STAGING_ACCOUNT_V3,
            &facts.config,
        ),
        (
            HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
            HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
            &facts.account_profile,
        ),
        (
            HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
            HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
            &facts.request_profile,
        ),
        (
            HOT_TRANSITION_RAW_ACCOUNT_V3,
            HOT_TRANSITION_STAGING_ACCOUNT_V3,
            &facts.transition,
        ),
        (
            HOT_EFFECT_RAW_ACCOUNT_V3,
            HOT_EFFECT_STAGING_ACCOUNT_V3,
            &facts.effect,
        ),
        (
            HOT_LIFECYCLE_RAW_ACCOUNT_V3,
            HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
            &facts.lifecycle,
        ),
        (
            HOT_STRATEGY_RAW_ACCOUNT_V3,
            HOT_STRATEGY_STAGING_ACCOUNT_V3,
            &facts.strategy,
        ),
        (
            HOT_PRODUCT_RAW_ACCOUNT_V3,
            HOT_PRODUCT_STAGING_ACCOUNT_V3,
            &input.fixed.product,
        ),
        (
            HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
            HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3,
            &input.fixed.result_domain,
        ),
        (
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            HOT_PORTFOLIO_STAGING_ACCOUNT_V3,
            &input.fixed.portfolio,
        ),
        (
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
            HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
            &input.fixed.linked_basis,
        ),
    ] {
        set(raw, fixed_readonly(finalized_raw(rent, record)))?;
        set(staging, fixed_readonly(vacant(record.staging)))?;
    }
    set(
        HOT_CAPABILITY_SEAL_ACCOUNT_V3,
        fixed_readonly(data_account(
            rent,
            facts.seal,
            input.waist.trading_program,
            facts.seal_bytes.clone(),
        )),
    )?;
    set(
        HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        fixed_readonly(external(
            input.waist.activation_cache,
            input.waist.registry_program,
        )),
    )?;
    set(
        HOT_CORE_PROGRAM_ACCOUNT_V3,
        fixed_readonly(program(input.waist.core_program)),
    )?;
    set(
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        fixed_readonly(external(
            input.fixed.core_programdata,
            solana_sdk_ids::bpf_loader_upgradeable::ID,
        )),
    )?;
    set(
        HOT_TRADING_PROGRAM_ACCOUNT_V3,
        fixed_readonly(program(input.waist.trading_program)),
    )?;
    set(
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
        fixed_readonly(external(
            input.fixed.trading_programdata,
            solana_sdk_ids::bpf_loader_upgradeable::ID,
        )),
    )?;
    set(
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        fixed_readonly(program(input.waist.registry_program)),
    )?;
    set(
        HOT_RENT_SYSVAR_ACCOUNT_V3,
        fixed_readonly(external(sysvar::rent::ID, sysvar::ID)),
    )?;
    set(
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        fixed_readonly(external(sysvar::instructions::ID, sysvar::ID)),
    )?;
    fixed
        .into_iter()
        .map(|value| value.ok_or(BuilderError::Profile(line!())))
        .collect()
}

#[cfg(test)]
mod tests {
    use dclutch_account_profile_contract::{
        v2::{
            AccountPrestateV2, HEADER_BYTES, OPERATION_BYTES, RULE_BYTES,
            encode::{
                AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
                AccountOperationInputV2, AccountPrivilegesV2, AccountProfileArtifactV2,
                AccountRuleInputV2, RegisterGeometryV2, ScalarCoordinateV2,
                encode_account_profile_v2_atomic,
            },
        },
        v3::{HEADER_BYTES_V3, encode_account_profile_v3_atomic},
    };

    use super::*;

    fn exact_base_profile() -> Vec<u8> {
        let rules = [
            AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, true, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, true, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 64,
                data_item_stride: 0,
            },
            AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, false),
                effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
        ];
        let operations = [AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(1),
            destination: ScalarCoordinateV2::common(0),
        }];
        let registers = RegisterGeometryV2 {
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        let mut scratch = vec![0_u8; HEADER_BYTES + 2 * RULE_BYTES + OPERATION_BYTES];
        let mut output = vec![0_u8; scratch.len()];
        encode_account_profile_v2_atomic(
            AccountProfileArtifactV2::RuntimeTail,
            &rules,
            &[],
            &operations,
            &[],
            registers,
            &mut scratch,
            &mut output,
        )
        .expect("exact V2 profile");
        output
    }

    fn exact_successor_profile(base: &[u8]) -> Vec<u8> {
        let mut scratch = vec![0_u8; HEADER_BYTES_V3 + base.len()];
        let mut output = vec![0_u8; scratch.len()];
        encode_account_profile_v3_atomic(base, &[], &mut scratch, &mut output)
            .expect("exact V3 profile");
        output
    }

    #[test]
    fn schema_selects_v2_or_full_v3_base_execution_view() {
        let base = exact_base_profile();
        let successor = exact_successor_profile(&base);
        assert_eq!(
            decode_execution_account_profile(ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2, &base)
                .expect("V2 execution view")
                .bytes(),
            base
        );
        assert_eq!(
            decode_execution_account_profile(ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V3, &successor,)
                .expect("V3 base execution view")
                .bytes(),
            base
        );
        assert_ne!(successor, base);
    }

    #[test]
    fn schema_refuses_unknown_malformed_and_hybrid_profiles() {
        let base = exact_base_profile();
        let successor = exact_successor_profile(&base);
        assert_eq!(
            decode_execution_account_profile([0x71; 32], &base),
            Err(BuilderError::Artifact)
        );
        assert_eq!(
            decode_execution_account_profile(ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V2, &successor),
            Err(BuilderError::Artifact)
        );
        assert_eq!(
            decode_execution_account_profile(ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V3, &base),
            Err(BuilderError::Artifact)
        );
        let mut malformed = successor;
        malformed[12] = 1;
        assert_eq!(
            decode_execution_account_profile(ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V3, &malformed),
            Err(BuilderError::Artifact)
        );
        let mut malformed_base = exact_successor_profile(&base);
        let embedded_magic = HEADER_BYTES_V3;
        malformed_base[embedded_magic] ^= 1;
        assert_eq!(
            decode_execution_account_profile(ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V3, &malformed_base,),
            Err(BuilderError::Artifact)
        );

        // The successor never weakens the embedded base profile's prestate.
        assert_eq!(
            AccountProfileV2::decode(&base)
                .expect("exact base")
                .rule(false, 0)
                .expect("fixed rule")
                .prestate(),
            AccountPrestateV2::Exact
        );
    }
}
