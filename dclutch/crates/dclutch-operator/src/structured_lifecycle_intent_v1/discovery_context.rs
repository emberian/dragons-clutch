//! Native on-chain traversal for the selected Structured lifecycle constructor.

use super::{discovery::*, types::*};
use crate::observation::decode_clock;
use dclutch_claims::{
    composition::{
        COMPOSITION_EXPOSURE_SCHEMA_ID_V3, CompositionExposureBundleV3, RecordAdmissionV3,
    },
    rational::RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    rational_kernel::{
        DESCRIPTOR_MAGIC_V3, DESCRIPTOR_MARKET_ID_OFFSET, DescriptorAdmissionV2,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3, RepresentationDescriptorV2,
    },
    rational_lifecycle::hot_v6::structured_lifecycle_action_selector_v1,
    structured_kernel::STRUCTURED_CAPABILITY_KIND_ID_V2,
};
use dclutch_core_contract::ContentId;
use dclutch_market::{
    CoreState, MarketCoreStateSeedsV2,
    capability_manifest::{
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityEntryV1, CapabilityManifestV1,
    },
    capability_program::{
        CapabilityRootHeaderV1, SelectedRecordBumpsV1,
        set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
        v4::{CapabilityProgramV4, CapabilityRootAccountV4},
    },
};
use dclutch_product::{
    admission::{
        PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
        RESULT_DOMAIN_SCHEMA_ID_V2,
    },
    payoff::{
        registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        runtime_v3::{
            BASIS_MAGIC_V3, BASIS_PRODUCT_ID_OFFSET_V3, ProductBasisV3, semantic_basis_id_v3,
        },
    },
};
use dclutch_registry::{
    ActivatedExecutionReleaseSetViewV1, ActivatedRoleV1,
    activation_auth_v1::authenticate_activated_role_v1,
    release_set::{CapabilityExecutionSelectionV1, ExecutionRoleV1},
    svm::{LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramV3View},
};
use dclutch_release_tool::CheckedExecutionReleaseSetV1;
use solana_program::{account_info::AccountInfo, clock::Clock, hash::hash, pubkey::Pubkey};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

/// Located native traversal failure; nested decoder causes remain visible.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructuredLifecycleContextErrorV1 {
    /// Supplied account/record corpus refused.
    Discovery(StructuredLifecycleDiscoveryErrorV1),
    /// A semantic decoder refused at its named stage.
    Native {
        /// Decoder or authentication boundary.
        stage: &'static str,
        /// Original native failure.
        cause: String,
    },
    /// Exact contextual equality or selection did not hold.
    Join(&'static str),
}
impl From<StructuredLifecycleDiscoveryErrorV1> for StructuredLifecycleContextErrorV1 {
    fn from(value: StructuredLifecycleDiscoveryErrorV1) -> Self {
        Self::Discovery(value)
    }
}
type Result<T> = core::result::Result<T, StructuredLifecycleContextErrorV1>;
fn native<T, E: core::fmt::Debug>(
    stage: &'static str,
    value: core::result::Result<T, E>,
) -> Result<T> {
    value.map_err(|error| StructuredLifecycleContextErrorV1::Native {
        stage,
        cause: format!("{error:?}"),
    })
}
fn join(condition: bool, message: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(StructuredLifecycleContextErrorV1::Join(message))
    }
}
fn full(address: [u8; 32]) -> StructuredLifecycleAccountRequestV1 {
    StructuredLifecycleAccountRequestV1 {
        address,
        data_slice: None,
    }
}

/// One checked activated role and the actual Loader ProgramData it names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredLifecycleRoleV1 {
    /// Semantic execution role.
    pub role: ExecutionRoleV1,
    /// Exact executable Program address.
    pub program: [u8; 32],
    /// Exact Loader ProgramData address.
    pub programdata: [u8; 32],
    /// Activated release after current deployment authentication.
    pub activated: ActivatedRoleV1,
}

/// Complete immutable selection and physical coordinates for native planning.
#[derive(Clone, Debug)]
pub struct StructuredLifecycleContextV1 {
    /// Decoded, canonically addressed Core Market.
    pub market: CoreState,
    /// Same-bank Clock used by all account observations.
    pub clock: Clock,
    /// Native-decoded offline checked release-set evidence.
    pub checked_release: CheckedExecutionReleaseSetV1,
    /// Currently authenticated roles used by this lifecycle.
    pub roles: Vec<StructuredLifecycleRoleV1>,
    /// Canonical Trading root account.
    pub root: [u8; 32],
    /// Persisted selection, including authenticated bump hints.
    pub root_header: CapabilityRootHeaderV1,
    /// Exact selected manifest entry.
    pub entry: CapabilityEntryV1,
    /// Market-selected manifest raw/staging pair.
    pub manifest: StructuredLifecycleFinalizedRecordV1,
    /// Selected immutable ProgramSet raw/staging pair.
    pub program_set: StructuredLifecycleFinalizedRecordV1,
    /// Realm/release Token behavior config raw/staging pair.
    pub config: StructuredLifecycleFinalizedRecordV1,
    /// Selected V6 descriptor, account profile, request profile, lifecycle, strategy,
    /// transition and effect records, in the canonical builder order.
    pub action_records: [StructuredLifecycleFinalizedRecordV1; 7],
    /// Selected per-Market representation descriptor.
    pub representation_descriptor: StructuredLifecycleFinalizedRecordV1,
    /// Claims-derived descriptor authority.
    pub representation_authority: [u8; 32],
    /// Descriptor-authenticated exposure record.
    pub exposure: StructuredLifecycleFinalizedRecordV1,
    /// Core-selected Product record.
    pub product: StructuredLifecycleFinalizedRecordV1,
    /// Product-selected ResultDomain record.
    pub result_domain: StructuredLifecycleFinalizedRecordV1,
    /// Product-selected Portfolio record.
    pub portfolio: StructuredLifecycleFinalizedRecordV1,
    /// Exposure-matching native Product basis record.
    pub basis: StructuredLifecycleFinalizedRecordV1,
    /// Realm record named by the Core identity.
    pub realm: StructuredLifecycleFinalizedRecordV1,
    /// Authenticated selected receipt/support choice.
    pub selection: StructuredLifecycleCapabilityV1,
}

/// Native discovery either asks for more observations/choices or returns full context.
#[derive(Clone, Debug)]
pub enum StructuredLifecycleContextProgressV1 {
    /// Browser/CLI acquisition or explicit user selection is required.
    Continue(StructuredLifecyclePlanningV1),
    /// Every immutable record and selected deployment is ready for construction.
    Ready(StructuredLifecycleContextV1),
}

fn needs(
    corpus: &StructuredLifecycleCorpusV1<'_>,
    requests: &[StructuredLifecycleAccountRequestV1],
) -> Result<Option<StructuredLifecycleContextProgressV1>> {
    let requests = corpus.missing(requests)?;
    Ok(
        (!requests.is_empty()).then_some(StructuredLifecycleContextProgressV1::Continue(
            StructuredLifecyclePlanningV1::Discover {
                requests,
                scans: vec![],
            },
        )),
    )
}
fn need_scan(
    corpus: &StructuredLifecycleCorpusV1<'_>,
    scan: &StructuredLifecycleProgramScanV1,
) -> Option<StructuredLifecycleContextProgressV1> {
    corpus.scan(scan).is_none().then(|| {
        StructuredLifecycleContextProgressV1::Continue(StructuredLifecyclePlanningV1::Discover {
            requests: vec![],
            scans: vec![scan.clone()],
        })
    })
}
fn record(
    corpus: &StructuredLifecycleCorpusV1<'_>,
    registry: [u8; 32],
    schema: [u8; 32],
    digest: [u8; 32],
    timestamp: i64,
) -> Result<StructuredLifecycleFinalizedRecordV1> {
    Ok(corpus.finalized_record(
        registry,
        structured_lifecycle_record_key_v1(registry, schema, digest)?,
        timestamp,
    )?)
}
fn record_requests(
    registry: [u8; 32],
    pairs: &[([u8; 32], [u8; 32])],
) -> Result<Vec<StructuredLifecycleAccountRequestV1>> {
    let mut output = Vec::new();
    for (schema, content) in pairs {
        output.extend(structured_lifecycle_record_key_v1(registry, *schema, *content)?.requests());
    }
    Ok(output)
}

struct OwnedAccountInfo {
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data: Vec<u8>,
    executable: bool,
}
impl OwnedAccountInfo {
    fn new(address: [u8; 32], value: &StructuredLifecycleAccountValueV1) -> Self {
        Self {
            key: Pubkey::new_from_array(address),
            owner: Pubkey::new_from_array(value.owner),
            lamports: value.lamports,
            data: value.data.clone(),
            executable: value.executable,
        }
    }
    fn info(&mut self) -> AccountInfo<'_> {
        AccountInfo::new(
            &self.key,
            false,
            false,
            &mut self.lamports,
            &mut self.data,
            &self.owner,
            self.executable,
        )
    }
}

fn authenticate_roles(
    corpus: &StructuredLifecycleCorpusV1<'_>,
    programs: &StructuredLifecycleProgramsV1,
    release_set: [u8; 32],
    checked: CheckedExecutionReleaseSetV1,
) -> Result<Vec<StructuredLifecycleRoleV1>> {
    let cache = corpus.present(programs.activation_cache)?;
    let activated = native(
        "activation cache",
        ActivatedExecutionReleaseSetViewV1::decode(&cache.data),
    )?;
    join(
        native(
            "activation release ID",
            activated.execution_release_set_id(),
        )?
        .to_bytes()
            == release_set,
        "Activation cache selects another release set",
    )?;
    join(
        native("checked release ID", checked.execution_release_set_id())?.to_bytes() == release_set,
        "Deployment manifest selects another release set",
    )?;
    let mut output = Vec::new();
    for (role, program) in [
        (ExecutionRoleV1::Core, programs.core),
        (ExecutionRoleV1::Claims, programs.claims),
        (ExecutionRoleV1::Trading, programs.trading),
        (ExecutionRoleV1::Custody, programs.custody),
    ] {
        let binding = native("activated role", activated.role(role))?;
        let checked_role = checked.artifacts()[role.role_index()];
        join(
            binding.release() == checked_role,
            "Checked role artifact differs from activation",
        )?;
        join(
            binding.release().program().to_bytes() == program,
            "Selected deployment program differs from activated role",
        )?;
        let programdata = binding.release().programdata();
        let data =
            corpus
                .get(programdata)
                .ok_or(StructuredLifecycleDiscoveryErrorV1::MissingAccount(
                    programdata,
                ))?;
        if let Some(slice) = data.request.data_slice {
            join(
                slice.offset == 0
                    && usize::try_from(slice.length).ok()
                        == Some(LOADER_V3_PROGRAMDATA_METADATA_BYTES),
                "ProgramData observation is not the canonical Loader header",
            )?;
        }
        let data_value =
            data.value
                .as_ref()
                .ok_or(StructuredLifecycleDiscoveryErrorV1::AbsentAccount(
                    programdata,
                ))?;
        let mut registry_info =
            OwnedAccountInfo::new(programs.registry, corpus.present(programs.registry)?);
        let mut cache_info = OwnedAccountInfo::new(programs.activation_cache, cache);
        let mut program_info = OwnedAccountInfo::new(program, corpus.present(program)?);
        let mut programdata_info = OwnedAccountInfo::new(programdata, data_value);
        native(
            "current role deployment",
            authenticate_activated_role_v1(
                &registry_info.info(),
                &cache_info.info(),
                &release_set,
                role,
                &program_info.info(),
                &programdata_info.info(),
            ),
        )?;
        output.push(StructuredLifecycleRoleV1 {
            role,
            program,
            programdata,
            activated: binding,
        });
    }
    Ok(output)
}

/// Traverse Market, selected Registry records and descriptor support without RPC or reports.
pub fn discover_structured_lifecycle_context_v1(
    intent: &StructuredLifecycleIntentV1,
    programs: &StructuredLifecycleProgramsV1,
    snapshot: &StructuredLifecycleSnapshotV1,
) -> Result<StructuredLifecycleContextProgressV1> {
    let corpus = StructuredLifecycleCorpusV1::new(snapshot)?;
    let checked = native(
        "checked deployment manifest",
        CheckedExecutionReleaseSetV1::decode(&programs.checked_execution_release_set),
    )?;
    let initial = [
        intent.market,
        intent.payer,
        programs.core,
        programs.registry,
        programs.claims,
        programs.trading,
        programs.custody,
        programs.rent_program,
        programs.activation_cache,
        sysvar::clock::ID.to_bytes(),
        sysvar::rent::ID.to_bytes(),
        sysvar::instructions::ID.to_bytes(),
        system_program::ID.to_bytes(),
        spl_token_2022_interface::ID.to_bytes(),
    ]
    .map(full);
    if let Some(progress) = needs(&corpus, &initial)? {
        return Ok(progress);
    }
    let clock = native(
        "Clock",
        decode_clock(&corpus.observed(sysvar::clock::ID.to_bytes(), 0)?),
    )?;
    join(
        clock.slot == snapshot.slot && snapshot.slot != 0,
        "Account snapshot and Clock do not share a finalized slot",
    )?;
    let account = corpus.present(intent.market)?;
    join(
        account.owner == programs.core && !account.executable,
        "Market account is not owned by the selected Core program",
    )?;
    let market = native("Core Market", CoreState::decode(&account.data))?;
    join(
        Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
            &Pubkey::new_from_array(programs.core),
        )
        .0
        .to_bytes()
            == intent.market,
        "Market address differs from its canonical Core identity",
    )?;
    join(
        market.identity.registry_program.to_bytes() == programs.registry,
        "Market selects another Registry program",
    )?;
    let release_set = market.identity.selected_release_set.to_bytes();
    let mut requests = Vec::new();
    for program in [
        programs.core,
        programs.claims,
        programs.trading,
        programs.custody,
    ] {
        let account = corpus.present(program)?;
        join(
            account.owner == bpf_loader_upgradeable::ID.to_bytes() && account.executable,
            "Selected role is not a Loader-owned executable program",
        )?;
        let programdata =
            native("Loader Program", ProgramV3View::parse(&account.data))?.programdata();
        requests.push(StructuredLifecycleAccountRequestV1 {
            address: programdata,
            data_slice: Some(StructuredLifecycleDataSliceV1 {
                offset: 0,
                length: u32::try_from(LOADER_V3_PROGRAMDATA_METADATA_BYTES)
                    .expect("fixed Loader header"),
            }),
        });
    }
    let manifest_pair = (
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market.identity.capability_manifest.to_bytes(),
    );
    let product_pair = (
        PRODUCT_RECORD_SCHEMA_ID_V2,
        market.identity.product_record.to_bytes(),
    );
    let realm_pair = (
        dclutch_market::realm::REALM_SCHEMA_RELEASE_ID_V1,
        market.identity.realm_id.to_bytes(),
    );
    requests.extend(record_requests(
        programs.registry,
        &[manifest_pair, product_pair, realm_pair],
    )?);
    if let Some(progress) = needs(&corpus, &requests)? {
        return Ok(progress);
    }
    let roles = authenticate_roles(&corpus, programs, release_set, checked)?;
    let manifest_record = record(
        &corpus,
        programs.registry,
        manifest_pair.0,
        manifest_pair.1,
        clock.unix_timestamp,
    )?;
    let manifest = native(
        "Capability manifest",
        CapabilityManifestV1::decode(&manifest_record.raw.data),
    )?;
    let product = record(
        &corpus,
        programs.registry,
        product_pair.0,
        product_pair.1,
        clock.unix_timestamp,
    )?;
    let product_view = native("Product record", ProductRecordV2::decode(&product.raw.data))?;
    join(
        product_view.product_id().to_bytes() == market.identity.product_id.to_bytes(),
        "Product record differs from Market Product identity",
    )?;
    let realm = record(
        &corpus,
        programs.registry,
        realm_pair.0,
        realm_pair.1,
        clock.unix_timestamp,
    )?;
    let mut candidates = Vec::new();
    for index in 0..manifest.entry_count() {
        let entry = native("Capability manifest entry", manifest.entry(index))?;
        if entry.kind_id().to_bytes() != STRUCTURED_CAPABILITY_KIND_ID_V2 {
            continue;
        }
        let selection = native(
            "Capability selection",
            CapabilityExecutionSelectionV1::from_bytes(
                index,
                manifest_pair.1,
                entry.kind_id().to_bytes(),
                entry.release_id().to_bytes(),
                entry.config_id().to_bytes(),
            ),
        )?;
        let header = native(
            "Capability root header",
            CapabilityRootHeaderV1::new(
                native("Release content ID", ContentId::new(release_set))?,
                intent.market,
                market.identity.generation,
                selection,
                SelectedRecordBumpsV1::default(),
            ),
        )?;
        let root = Pubkey::find_program_address(
            &header.seeds().as_slices(),
            &Pubkey::new_from_array(programs.trading),
        )
        .0
        .to_bytes();
        if intent.selected_capability.is_some_and(|pin| pin != root) {
            continue;
        }
        candidates.push((root, entry, selection));
    }
    join(
        !candidates.is_empty(),
        "Choose a Structured capability published for this Market",
    )?;
    let mut requests = Vec::new();
    for (root, entry, _) in &candidates {
        requests.push(full(*root));
        requests.extend(record_requests(
            programs.registry,
            &[(
                CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
                entry.release_id().to_bytes(),
            )],
        )?);
    }
    if let Some(progress) = needs(&corpus, &requests)? {
        return Ok(progress);
    }
    let selector =
        structured_lifecycle_action_selector_v1(STRUCTURED_CAPABILITY_KIND_ID_V2, intent.action)
            .ok_or(StructuredLifecycleContextErrorV1::Join(
                "Unsupported Structured lifecycle action",
            ))?;
    let mut eligible = Vec::new();
    for (root, entry, selection) in candidates {
        let set_record = record(
            &corpus,
            programs.registry,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            entry.release_id().to_bytes(),
            clock.unix_timestamp,
        )?;
        let set = native(
            "Capability ProgramSet",
            CapabilityProgramSetV2::decode_selected(
                entry.release_id().to_bytes(),
                set_record.key.content,
                &set_record.raw.data,
            ),
        )?;
        let mut selected = None;
        for index in 0..set.entry_count() {
            let candidate = native("ProgramSet entry", set.entry(index))?;
            if candidate.selector() == selector {
                selected = Some(candidate.descriptor());
            }
        }
        if let Some(descriptor) = selected {
            eligible.push((root, entry, selection, set_record, descriptor));
        }
    }
    join(
        !eligible.is_empty(),
        "The selected Structured capability does not support this lifecycle action",
    )?;
    if eligible.len() > 1 {
        return Ok(StructuredLifecycleContextProgressV1::Continue(
            StructuredLifecyclePlanningV1::SelectCapability {
                capabilities: eligible
                    .iter()
                    .map(
                        |(root, entry, selection, _, _)| StructuredLifecycleRootChoiceV1 {
                            capability: *root,
                            program_set: entry.release_id().to_bytes(),
                            entry_index: selection.entry_index(),
                        },
                    )
                    .collect(),
            },
        ));
    }
    let (root, entry, selection, program_set, action_reference) =
        eligible.pop().expect("one eligible capability");
    let action_key = structured_lifecycle_record_key_v1(
        programs.registry,
        action_reference.schema().to_bytes(),
        action_reference.program().to_bytes(),
    )?;
    if let Some(progress) = needs(&corpus, &action_key.requests())? {
        return Ok(progress);
    }
    let action_record =
        corpus.finalized_record(programs.registry, action_key, clock.unix_timestamp)?;
    join(
        action_key.schema == dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
        "Selected lifecycle descriptor uses another schema",
    )?;
    let action = native(
        "Lifecycle capability descriptor",
        CapabilityProgramV4::decode(&action_record.raw.data),
    )?;
    native(
        "Lifecycle manifest selection",
        action.validate_selection(selection, entry),
    )?;
    let root_account = corpus.present(root)?;
    join(
        root_account.owner == programs.trading && !root_account.executable,
        "Activate this Market's Structured capability before preparing its receipts",
    )?;
    let root_view = native(
        "Structured capability root",
        CapabilityRootAccountV4::decode(&root_account.data, action),
    )?;
    let root_header = root_view.header();
    let persisted = root_header.selection();
    join(
        root_header.release_set().to_bytes() == release_set
            && root_header.market() == intent.market
            && root_header.generation() == market.identity.generation,
        "Capability root belongs to another Market generation",
    )?;
    join(
        persisted.entry_index() == selection.entry_index()
            && persisted.manifest() == selection.manifest()
            && persisted.kind() == selection.kind()
            && persisted.capability_release() == selection.capability_release()
            && persisted.config() == selection.config(),
        "Capability root differs from the selected manifest entry",
    )?;
    native(
        "Structured resource root",
        dclutch_trading::structured_root_v2::StructuredCapabilityRootV2::decode(root_view.state()),
    )?;
    let config_pair = (
        action.config_schema().to_bytes(),
        entry.config_id().to_bytes(),
    );
    let artifacts = [
        action.account_profile(),
        action.request_profile(),
        action.lifecycle(),
        action.strategy(),
        action.transition(),
        action.effect(),
    ];
    let mut pairs = vec![config_pair];
    pairs.extend(artifacts.map(|reference| {
        (
            reference.schema().to_bytes(),
            reference.program().to_bytes(),
        )
    }));
    pairs.extend([
        (
            RESULT_DOMAIN_SCHEMA_ID_V2,
            product_view.result_domain_digest().to_bytes(),
        ),
        (
            PORTFOLIO_SCHEMA_ID_V2,
            product_view.portfolio_digest().to_bytes(),
        ),
    ]);
    if let Some(progress) = needs(&corpus, &record_requests(programs.registry, &pairs)?)? {
        return Ok(progress);
    }
    let config = record(
        &corpus,
        programs.registry,
        config_pair.0,
        config_pair.1,
        clock.unix_timestamp,
    )?;
    let mut action_records = vec![action_record];
    for reference in artifacts {
        action_records.push(record(
            &corpus,
            programs.registry,
            reference.schema().to_bytes(),
            reference.program().to_bytes(),
            clock.unix_timestamp,
        )?);
    }
    let action_records: [StructuredLifecycleFinalizedRecordV1; 7] = action_records
        .try_into()
        .expect("descriptor plus six artifacts");
    let result_domain = record(
        &corpus,
        programs.registry,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product_view.result_domain_digest().to_bytes(),
        clock.unix_timestamp,
    )?;
    let portfolio = record(
        &corpus,
        programs.registry,
        PORTFOLIO_SCHEMA_ID_V2,
        product_view.portfolio_digest().to_bytes(),
        clock.unix_timestamp,
    )?;
    discover_representation(
        intent,
        programs,
        &corpus,
        market,
        clock,
        checked,
        roles,
        root,
        root_header,
        entry,
        manifest_record,
        program_set,
        config,
        action_records,
        product,
        result_domain,
        portfolio,
        realm,
    )
}

fn descriptor_view<'a>(
    record: &'a StructuredLifecycleFinalizedRecordV1,
    claims: [u8; 32],
) -> Result<RepresentationDescriptorV2<'a>> {
    let authority = Pubkey::find_program_address(
        &[
            RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
            &record.key.content,
        ],
        &Pubkey::new_from_array(claims),
    )
    .0
    .to_bytes();
    native(
        "Representation descriptor",
        RepresentationDescriptorV2::decode(
            &record.raw.data,
            DescriptorAdmissionV2 {
                selected_descriptor_id: record.key.content,
                finalized_descriptor_id: record.key.content,
                recomputed_descriptor_digest: hash(&record.raw.data).to_bytes(),
                finalized_descriptor_digest: record.key.content,
                record_authenticated: true,
                derived_representation_authority: authority,
                authority_derivation_authenticated: true,
            },
        ),
    )
}

fn scan_for(
    program: [u8; 32],
    magic: &[u8],
    identity_offset: usize,
    identity: [u8; 32],
) -> Result<StructuredLifecycleProgramScanV1> {
    Ok(StructuredLifecycleProgramScanV1 {
        program,
        data_size: None,
        memcmp: vec![
            StructuredLifecycleMemcmpV1 {
                offset: 0,
                bytes: magic.to_vec(),
            },
            StructuredLifecycleMemcmpV1 {
                offset: u32::try_from(identity_offset).map_err(|_| {
                    StructuredLifecycleContextErrorV1::Join(
                        "Native scan offset exceeds its transport width",
                    )
                })?,
                bytes: identity.to_vec(),
            },
        ],
        data_slice: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn discover_representation(
    intent: &StructuredLifecycleIntentV1,
    programs: &StructuredLifecycleProgramsV1,
    corpus: &StructuredLifecycleCorpusV1<'_>,
    market: CoreState,
    clock: Clock,
    checked_release: CheckedExecutionReleaseSetV1,
    roles: Vec<StructuredLifecycleRoleV1>,
    root: [u8; 32],
    root_header: CapabilityRootHeaderV1,
    entry: CapabilityEntryV1,
    manifest: StructuredLifecycleFinalizedRecordV1,
    program_set: StructuredLifecycleFinalizedRecordV1,
    config: StructuredLifecycleFinalizedRecordV1,
    action_records: [StructuredLifecycleFinalizedRecordV1; 7],
    product: StructuredLifecycleFinalizedRecordV1,
    result_domain: StructuredLifecycleFinalizedRecordV1,
    portfolio: StructuredLifecycleFinalizedRecordV1,
    realm: StructuredLifecycleFinalizedRecordV1,
) -> Result<StructuredLifecycleContextProgressV1> {
    let release_set = market.identity.selected_release_set.to_bytes();
    let addresses = if let Some(descriptor) = intent.representation_descriptor {
        vec![
            structured_lifecycle_record_key_v1(
                programs.registry,
                REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
                descriptor,
            )?
            .raw,
        ]
    } else {
        let scan = scan_for(
            programs.registry,
            &DESCRIPTOR_MAGIC_V3,
            DESCRIPTOR_MARKET_ID_OFFSET,
            intent.market,
        )?;
        if let Some(progress) = need_scan(corpus, &scan) {
            return Ok(progress);
        }
        corpus
            .scan(&scan)
            .expect("acquired inventory")
            .addresses
            .clone()
    };
    if let Some(progress) = needs(
        corpus,
        &addresses.iter().copied().map(full).collect::<Vec<_>>(),
    )? {
        return Ok(progress);
    }
    let mut keys = Vec::new();
    for address in addresses {
        let Some(value) = corpus.full(address)? else {
            continue;
        };
        let key = structured_lifecycle_record_key_v1(
            programs.registry,
            REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            hash(&value.data).to_bytes(),
        )?;
        if key.raw == address {
            keys.push(key);
        } else if intent.representation_descriptor.is_some() {
            return Err(StructuredLifecycleContextErrorV1::Join(
                "Selected receipt descriptor bytes differ from their content address",
            ));
        }
    }
    if let Some(progress) = needs(
        corpus,
        &keys
            .iter()
            .flat_map(|key| key.requests())
            .collect::<Vec<_>>(),
    )? {
        return Ok(progress);
    }
    let mut descriptors = Vec::new();
    for key in keys {
        let candidate = corpus.finalized_record(programs.registry, key, clock.unix_timestamp);
        let record = match candidate {
            Ok(record) => record,
            Err(error) if intent.representation_descriptor.is_some() => return Err(error.into()),
            Err(_) => continue, // Inventories may include unfinished publication; it grants no choice.
        };
        let view = match descriptor_view(&record, programs.claims) {
            Ok(view) => view,
            Err(error) if intent.representation_descriptor.is_some() => return Err(error),
            Err(_) => continue,
        };
        let selected = view.market_id() == intent.market
            && view.release_set_id() == release_set
            && view.token_program() == spl_token_2022_interface::ID.to_bytes();
        if !selected {
            if intent.representation_descriptor.is_some() {
                return Err(StructuredLifecycleContextErrorV1::Join(
                    "Receipt descriptor belongs to another Market, release or Token program",
                ));
            }
            continue;
        }
        let seeds = native(
            "Receipt mint seeds",
            dclutch_claims::rational::RationalReceiptMintSeedsV2::new(
                view.graph_digest(),
                intent.market,
                release_set,
            ),
        )?;
        let mint = Pubkey::find_program_address(
            &seeds.as_slices(),
            &Pubkey::new_from_array(programs.claims),
        )
        .0
        .to_bytes();
        native(
            "Receipt mint identity",
            seeds.authenticate_address(mint, view.receipt_mint()),
        )?;
        let mut support = Vec::new();
        for coordinate in 0..view.outcome_count() {
            let coefficient = native("Descriptor coefficient", view.coefficient(coordinate))?;
            if coefficient != 0 {
                support.push(StructuredLifecycleSupportV1 {
                    coordinate,
                    coefficient,
                });
            }
        }
        let choice = StructuredLifecycleCapabilityV1 {
            capability: root,
            descriptor: key.content,
            receipt_mint: view.receipt_mint(),
            denominator: view.denominator(),
            support,
        };
        descriptors.push((record, choice));
    }
    join(
        !descriptors.is_empty(),
        "Publish a Structured receipt descriptor for this Market before preparing its resources",
    )?;
    if descriptors.len() > 1
        || (matches!(
            intent.action,
            dclutch_claims::rational_lifecycle::LifecycleActionV2::ActivateCoordinate
                | dclutch_claims::rational_lifecycle::LifecycleActionV2::RetireCoordinate
        ) && intent.coordinate.is_none())
    {
        return Ok(StructuredLifecycleContextProgressV1::Continue(
            StructuredLifecyclePlanningV1::Select {
                capabilities: descriptors.into_iter().map(|(_, choice)| choice).collect(),
            },
        ));
    }
    let (representation_descriptor, selection) =
        descriptors.pop().expect("one selected representation");
    let descriptor = descriptor_view(&representation_descriptor, programs.claims)?;
    if let Some(coordinate) = intent.coordinate {
        join(
            selection
                .support
                .iter()
                .any(|row| row.coordinate == coordinate),
            "Choose a nonzero coordinate from this receipt descriptor",
        )?;
    }
    let exposure_key = structured_lifecycle_record_key_v1(
        programs.registry,
        COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
        descriptor.graph_id(),
    )?;
    if let Some(progress) = needs(corpus, &exposure_key.requests())? {
        return Ok(progress);
    }
    let exposure_record =
        corpus.finalized_record(programs.registry, exposure_key, clock.unix_timestamp)?;
    let exposure = native(
        "Composition exposure",
        CompositionExposureBundleV3::decode(
            &exposure_record.raw.data,
            RecordAdmissionV3 {
                selected_id: exposure_key.content,
                finalized_id: exposure_key.content,
                recomputed_digest: hash(&exposure_record.raw.data).to_bytes(),
                finalized_digest: exposure_key.content,
                record_authenticated: true,
            },
        ),
    )?;
    native(
        "Descriptor exposure join",
        descriptor.authenticate_exposure(exposure),
    )?;
    join(
        exposure.market() == intent.market
            && exposure.release_set() == release_set
            && exposure.result_domain() == result_domain.key.content,
        "Representation exposure selects another Market, release or Product domain",
    )?;
    let basis_scan = scan_for(
        programs.registry,
        &BASIS_MAGIC_V3,
        BASIS_PRODUCT_ID_OFFSET_V3,
        market.identity.product_id.to_bytes(),
    )?;
    if let Some(progress) = need_scan(corpus, &basis_scan) {
        return Ok(progress);
    }
    let basis_addresses = corpus
        .scan(&basis_scan)
        .expect("basis inventory")
        .addresses
        .clone();
    if let Some(progress) = needs(
        corpus,
        &basis_addresses
            .iter()
            .copied()
            .map(full)
            .collect::<Vec<_>>(),
    )? {
        return Ok(progress);
    }
    let mut basis_keys = Vec::new();
    for address in basis_addresses {
        let Some(value) = corpus.full(address)? else {
            continue;
        };
        let Ok(basis) = ProductBasisV3::decode(&value.data) else {
            continue;
        };
        let Ok(semantic) = semantic_basis_id_v3(&value.data) else {
            continue;
        };
        if basis.product_id() != market.identity.product_id.to_bytes()
            || basis.result_domain_id() != result_domain.key.content
            || semantic != exposure.product_basis()
            || basis.basis_width() != exposure.product_width()
        {
            continue;
        }
        let key = structured_lifecycle_record_key_v1(
            programs.registry,
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            hash(&value.data).to_bytes(),
        )?;
        if key.raw == address {
            basis_keys.push(key);
        }
    }
    if let Some(progress) = needs(
        corpus,
        &basis_keys
            .iter()
            .flat_map(|key| key.requests())
            .collect::<Vec<_>>(),
    )? {
        return Ok(progress);
    }
    let mut bases = Vec::new();
    for key in basis_keys {
        if let Ok(basis) = corpus.finalized_record(programs.registry, key, clock.unix_timestamp) {
            bases.push(basis);
        }
    }
    join(
        bases.len() == 1,
        "Acquire the single finalized Product basis selected by this receipt exposure",
    )?;
    let representation_authority = descriptor.representation_authority();
    Ok(StructuredLifecycleContextProgressV1::Ready(
        StructuredLifecycleContextV1 {
            market,
            clock,
            checked_release,
            roles,
            root,
            root_header,
            entry,
            manifest,
            program_set,
            config,
            action_records,
            representation_descriptor,
            representation_authority,
            exposure: exposure_record,
            product,
            result_domain,
            portfolio,
            basis: bases.pop().expect("one authenticated basis"),
            realm,
            selection,
        },
    ))
}
