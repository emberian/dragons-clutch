//! Bundle assembly: the fixed Hot frame, the adoption loop, and the final
//! instruction and account set.

use dclutch_account_profile_contract::v2::AccountProfileV2;
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
    HOT_TRANSITION_STAGING_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
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
    artifacts::{ArtifactSetV1, DerivedArtifactsV1, DerivedRecordV1, derive_artifact_facts},
    frame::{BuiltAccountV1, LogicalFrameV1, data_account, external, pack_frame, program, vacant},
    profile_ops,
    registers::{
        ContentProjectionKeysV1, EngineInputV1, EngineOutputV1, ObservedAccountV1,
        SpanWidthInputV1, derive_dynamic_span_widths, run_engine,
    },
    routes::{DerivedAuthorityV1, derive_authority},
};

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
pub fn build_bundle(input: &BundleInputV1<'_>) -> Result<BuiltBundleV1, BuilderError> {
    let facts = derive_artifact_facts(input.set, input.waist, input.scenario.family_request)?;
    let profile =
        AccountProfileV2::decode(input.set.account_profile).map_err(|_| BuilderError::Artifact)?;
    let tail_count = input.scenario.tail_count;
    // The span widths come before the frame exists: they *are* the frame's
    // width. Derived from the artifacts and the family request, exactly as the
    // Hot executor derives them before it expands one account.
    let span_counts = derive_dynamic_span_widths(&SpanWidthInputV1 {
        profile,
        request_profile_bytes: input.set.request_profile,
        request_profile_schema: facts.request_profile.schema,
        effect_bytes: input.set.effect,
        strategy_bytes: input.set.strategy,
        waist: input.waist,
        tail_count,
        family_request: input.scenario.family_request,
        clock_slot: input.scenario.clock_slot,
    })?;
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
    .map_err(|_| BuilderError::Binding(line!()))?;
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
        let output = run_engine(&EngineInputV1 {
            profile,
            request_profile_bytes: input.set.request_profile,
            request_profile_schema: facts.request_profile.schema,
            lifecycle_bytes: input.set.lifecycle,
            transition_bytes: input.set.transition,
            effect_bytes: input.set.effect,
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
        })?;
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
    })
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
