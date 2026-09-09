//! Pure staged discovery of canonical Dealer liquidity account coordinates.
//!
//! RPC is deliberately absent.  A caller supplies an unordered corpus read at
//! one finalized slot; this module hostile-decodes the native protocol records,
//! derives the next addresses, and eventually returns the exact grouped input
//! accepted by the canonical Dealer instruction builders.

use std::{collections::BTreeSet, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_claims::{
    liability_basis_state_v2::LiabilityBasisMarketSeedsV2,
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_core_contract::ContentId;
use dclutch_market::{CoreState, MarketCoreStateSeedsV2};
use dclutch_market::{
    capability_manifest::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1},
    capability_program::{
        CapabilityRootHeaderV1, SelectedRecordBumpsV1,
        set_v2::{
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityDescriptorReferenceV2,
            CapabilityProgramSetV2,
        },
        v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
    },
};
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product::payoff::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{ProductBasisV3, semantic_basis_id_v3},
};
use dclutch_product::{PortfolioV2, ResultDomainV2, join_product_v2};
use dclutch_registry::{
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V2, ArtifactReleaseV2,
    activation::ActivatedExecutionReleaseSetViewV1,
    record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1},
    release_set::{CapabilityExecutionSelectionV1, ExecutionRoleV1},
    svm::ProgramV3View,
};
use dclutch_trading_sbf::dealer::{
    DEALER_KIND_PREIMAGE_V2, multi_lp::DEALER_LP_POSITION_PDA_DOMAIN_V3,
    lp_request::MultiLpRequestActionV3,
    obligation::DEALER_OBLIGATION_PDA_DOMAIN_V3,
};
use serde::{Deserialize, Serialize};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::wire::{
    AccountV1, CheckedReleaseV1, DecimalV1, InputV1, IntentV1, MAX_INPUT_BYTES_V1, ProgramsV1,
};

/// Exact discovery request transport.
pub const DISCOVERY_INPUT_FORMAT_V1: &str = "dclutch-dealer-liquidity-discovery-input-v1";
/// Exact discovery response transport.
pub const DISCOVERY_RESULT_FORMAT_V1: &str = "dclutch-dealer-liquidity-discovery-result-v1";
/// Provisional bounded number of accounts accepted by one discovery pass.
pub const MAX_DISCOVERY_ACCOUNTS_V1: usize = 256;
const LINKED_BASIS_QUERY_ID_V1: &str = "linked-basis-record-v1";

/// One exact memcmp clause produced by native discovery for RPC transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountQueryMemcmpV1 {
    /// Byte offset within account data.
    pub offset: u32,
    /// Canonical base64 bytes to compare.
    pub bytes_base64: String,
}

/// Bounded program-account observation requested by native discovery.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountQueryV1 {
    /// Stable native query identity echoed with the result.
    pub id: String,
    /// Exact account owner supplied to the RPC filter.
    pub owner: String,
    /// Optional exact account data width.
    pub data_size: Option<u32>,
    /// Native-produced byte filters; the transport must not reinterpret them.
    pub memcmp: Vec<AccountQueryMemcmpV1>,
}

/// Addresses returned by one completed native query.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountQueryResultV1 {
    /// Stable query identity from [`AccountQueryV1::id`].
    pub id: String,
    /// Every address returned by the bounded filtered RPC read.
    pub addresses: Vec<String>,
}

/// Unordered account corpus from one finalized RPC batch.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryRequestV1 {
    /// Exact discovery format.
    pub format: String,
    /// Typed Market, owner, payer, action, and expiry selection.
    pub intent: IntentV1,
    /// Slot shared by every supplied account, absent before the first read.
    pub snapshot_slot: Option<DecimalV1>,
    /// Checked role program identities.
    pub programs: ProgramsV1,
    /// Checked immutable release coordinates.
    pub checked_release: CheckedReleaseV1,
    /// Unordered account observations. Privilege bits must be false.
    pub accounts: Vec<AccountV1>,
    /// Completed native-produced program-account query address sets.
    pub query_results: Vec<AccountQueryResultV1>,
    /// Current exact rent minimum for the fixed LP Position width.
    pub lp_position_rent_lamports: Option<DecimalV1>,
}

/// One bounded discovery round or a complete canonical planner input.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DiscoveryResultV1 {
    /// More finalized observations are required.
    NeedsAccounts {
        /// Exact response format.
        format: &'static str,
        /// Canonical addresses missing from the supplied corpus.
        addresses: Vec<String>,
        /// Native-produced filtered program-account observations still required.
        queries: Vec<AccountQueryV1>,
        /// Lowest slot accepted for the next complete batch.
        minimum_finalized_slot: DecimalV1,
        /// The canonical Clock account must be present and decoded natively.
        require_clock: bool,
        /// The native fixed-width LP rent quote must be supplied.
        require_lp_position_rent: bool,
    },
    /// All native traversal and address derivation completed.
    Complete {
        /// Exact response format.
        format: &'static str,
        /// Sole input accepted by the native request and Hot builders.
        input: InputV1,
    },
}

/// Run one pure discovery round over caller-supplied finalized observations.
pub fn discover_dealer_liquidity_json_v1(source: &str) -> Result<String, String> {
    if source.is_empty() || source.len() > MAX_INPUT_BYTES_V1 {
        return Err("Dealer discovery input exceeds the bounded transport size".to_owned());
    }
    let request: DiscoveryRequestV1 =
        serde_json::from_str(source).map_err(|error| format!("Dealer discovery input: {error}"))?;
    let result = discover(&request)?;
    serde_json::to_string(&result).map_err(|error| format!("Dealer discovery output: {error}"))
}

/// Discover the next exact account set from a strict supplied corpus.
pub fn discover(request: &DiscoveryRequestV1) -> Result<DiscoveryResultV1, String> {
    if request.format != DISCOVERY_INPUT_FORMAT_V1 {
        return Err("Dealer discovery input format differs".to_owned());
    }
    if request.accounts.len() > MAX_DISCOVERY_ACCOUNTS_V1 {
        return Err("Dealer discovery exceeds 256 account rows".to_owned());
    }
    if request.query_results.len() > 4
        || request
            .query_results
            .iter()
            .any(|result| result.addresses.len() > MAX_DISCOVERY_ACCOUNTS_V1)
    {
        return Err("Dealer discovery query results exceed their bounded width".to_owned());
    }
    let intent_market = parse_key(&request.intent.market, "Market")?;
    let intent_owner = parse_key(&request.intent.owner, "LP owner")?;
    let intent_payer = parse_key(&request.intent.payer, "payer")?;
    if intent_owner == Pubkey::default() || intent_payer == Pubkey::default() {
        return Err("Dealer owner and payer must be nonzero".to_owned());
    }
    let programs = ParsedProgramsV1::parse(&request.programs)?;
    let corpus = CorpusV1::parse(request)?;
    let initial = [
        intent_market,
        sysvar::clock::ID,
        sysvar::rent::ID,
        sysvar::instructions::ID,
        system_program::ID,
        programs.core,
        programs.registry,
        programs.trading,
        programs.claims,
        programs.custody,
    ];
    if let Some(result) = needs(&corpus, &initial, request)? {
        return Ok(result);
    }

    let clock_observed = corpus.observed(sysvar::clock::ID, 0)?;
    let clock = dclutch_operator::observation::decode_clock(&clock_observed)
        .map_err(|_| "Dealer Clock observation refused".to_owned())?;
    let snapshot_slot = request
        .snapshot_slot
        .as_ref()
        .ok_or_else(|| "Dealer account corpus omits its finalized slot".to_owned())?
        .0;
    if clock.slot != snapshot_slot {
        return Err("Dealer Clock slot differs from the finalized account batch".to_owned());
    }
    let observation = Observation {
        slot: snapshot_slot,
        unix_timestamp: clock.unix_timestamp,
        finality: Finality::Finalized,
    };
    let market_account = corpus.observed(intent_market, clock.unix_timestamp)?;
    if market_account.owner != programs.core || market_account.executable {
        return Err("Dealer Market owner differs from checked Core".to_owned());
    }
    let market = CoreState::decode(&market_account.data)
        .map_err(|_| "Dealer Market state refused".to_owned())?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        &programs.core,
    )
    .0;
    if expected_market != intent_market
        || market.identity.registry_program.to_bytes() != programs.registry.to_bytes()
    {
        return Err("Dealer Market identity or Registry selection differs".to_owned());
    }
    let release_set = decode_lower_hex_32(&request.checked_release.release_set, "release set")?;
    if market.identity.selected_release_set.to_bytes() != release_set {
        return Err("Dealer Market selected another execution release set".to_owned());
    }
    if request.checked_release.generation.0 != 0
        && request.checked_release.generation.0 != market.identity.generation
    {
        return Err("Dealer checked generation is stale".to_owned());
    }

    let mut next = Vec::new();
    for program in programs.roles() {
        let account = corpus.observed(program, clock.unix_timestamp)?;
        if account.owner != bpf_loader_upgradeable::ID || !account.executable {
            return Err("Dealer role program deployment refused".to_owned());
        }
        let programdata = ProgramV3View::parse(&account.data)
            .map_err(|_| "Dealer role Program account refused".to_owned())?
            .programdata();
        push_unique(&mut next, Pubkey::new_from_array(programdata));
    }
    let registry = programs.registry;
    let manifest_digest = market.identity.capability_manifest.to_bytes();
    push_record_pair(
        &mut next,
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_digest,
    );
    push_record_pair(
        &mut next,
        registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        market.identity.product_record.to_bytes(),
    );
    push_record_pair(
        &mut next,
        registry,
        dclutch_market::realm::REALM_SCHEMA_RELEASE_ID_V1,
        market.identity.realm_id.to_bytes(),
    );
    push_unique(
        &mut next,
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &registry).0,
    );
    if let Some(result) = needs(&corpus, &next, request)? {
        return Ok(result);
    }

    let manifest_raw = record_address(
        registry,
        RAW_RECORD_PDA_SEED_V1,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_digest,
    );
    let manifest_account = corpus.observed(manifest_raw, clock.unix_timestamp)?;
    require_record(
        &corpus,
        &manifest_account,
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_digest,
    )?;
    let manifest = CapabilityManifestV1::decode(&manifest_account.data)
        .map_err(|_| "Dealer CapabilityManifest refused".to_owned())?;
    let dealer_kind = hash(DEALER_KIND_PREIMAGE_V2).to_bytes();
    let mut selected = None;
    for index in 0..manifest.entry_count() {
        let entry = manifest
            .entry(index)
            .map_err(|_| "Dealer CapabilityManifest entry refused".to_owned())?;
        if entry.kind_id().to_bytes() == dealer_kind {
            if selected.is_some() {
                return Err("Dealer CapabilityManifest contains duplicate Dealer kinds".to_owned());
            }
            selected = Some((index, entry));
        }
    }
    let (entry_index, entry) = selected
        .ok_or_else(|| "Dealer capability is absent from the selected Market".to_owned())?;
    let selection = CapabilityExecutionSelectionV1::from_bytes(
        entry_index,
        manifest_digest,
        dealer_kind,
        entry.release_id().to_bytes(),
        entry.config_id().to_bytes(),
    )
    .map_err(|_| "Dealer capability selection refused".to_owned())?;
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set).map_err(|_| "Dealer release set is zero".to_owned())?,
        intent_market.to_bytes(),
        market.identity.generation,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .map_err(|_| "Dealer root header refused".to_owned())?;
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &programs.trading).0;
    let obligation = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, root.as_ref()],
        &programs.trading,
    )
    .0;
    let lp_position = Pubkey::find_program_address(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            root.as_ref(),
            intent_owner.as_ref(),
        ],
        &programs.trading,
    )
    .0;
    let claims_market = Pubkey::find_program_address(
        &LiabilityBasisMarketSeedsV2::new(intent_market.to_bytes())
            .map_err(|_| "Dealer Claims aggregate seeds refused".to_owned())?
            .as_slices(),
        &programs.claims,
    )
    .0;
    let dealer_claims = protocol_position(claims_market, root, programs.claims)?;
    let lp_claims = protocol_position(claims_market, intent_owner, programs.claims)?;

    let product_raw = record_address(
        registry,
        RAW_RECORD_PDA_SEED_V1,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        market.identity.product_record.to_bytes(),
    );
    let product_account = corpus.observed(product_raw, clock.unix_timestamp)?;
    require_record(
        &corpus,
        &product_account,
        registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        market.identity.product_record.to_bytes(),
    )?;
    let product = ProductRecordV2::decode(&product_account.data)
        .map_err(|_| "Dealer Product record refused".to_owned())?;
    let mut base = vec![
        root,
        obligation,
        lp_position,
        claims_market,
        dealer_claims,
        lp_claims,
    ];
    push_record_pair(
        &mut base,
        registry,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        entry.release_id().to_bytes(),
    );
    push_record_pair(
        &mut base,
        registry,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product.result_domain_digest().to_bytes(),
    );
    push_record_pair(
        &mut base,
        registry,
        PORTFOLIO_SCHEMA_ID_V2,
        product.portfolio_digest().to_bytes(),
    );
    if let Some(result) = needs(&corpus, &base, request)? {
        return Ok(result);
    }

    let result_domain_raw = record_address(
        registry,
        RAW_RECORD_PDA_SEED_V1,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product.result_domain_digest().to_bytes(),
    );
    let result_domain_account = corpus.observed(result_domain_raw, clock.unix_timestamp)?;
    require_record(
        &corpus,
        &result_domain_account,
        registry,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product.result_domain_digest().to_bytes(),
    )?;
    let portfolio_raw = record_address(
        registry,
        RAW_RECORD_PDA_SEED_V1,
        PORTFOLIO_SCHEMA_ID_V2,
        product.portfolio_digest().to_bytes(),
    );
    let portfolio_account = corpus.observed(portfolio_raw, clock.unix_timestamp)?;
    require_record(
        &corpus,
        &portfolio_account,
        registry,
        PORTFOLIO_SCHEMA_ID_V2,
        product.portfolio_digest().to_bytes(),
    )?;
    let result_domain = ResultDomainV2::decode(&result_domain_account.data)
        .map_err(|_| "Dealer ResultDomain record refused".to_owned())?;
    let portfolio = PortfolioV2::decode(&portfolio_account.data)
        .map_err(|_| "Dealer Portfolio record refused".to_owned())?;
    let product_join = join_product_v2(
        product.result_domain_digest(),
        product.portfolio_digest(),
        result_domain,
        portfolio,
    )
    .map_err(|_| "Dealer Product graph composition refused".to_owned())?;
    if product.product_id().to_bytes() != market.identity.product_id.to_bytes()
        || product_join.product_id != product.product_id()
    {
        return Err("Dealer Product identity differs from the Market".to_owned());
    }

    let basis_query = AccountQueryV1 {
        id: LINKED_BASIS_QUERY_ID_V1.to_owned(),
        owner: registry.to_string(),
        data_size: None,
        memcmp: vec![AccountQueryMemcmpV1 {
            offset: u32::try_from(dclutch_product::payoff::runtime_v3::BASIS_MAGIC_OFFSET_V3)
                .map_err(|_| "Dealer linked-basis query offset overflowed".to_owned())?,
            bytes_base64: STANDARD.encode(dclutch_product::payoff::runtime_v3::BASIS_MAGIC_V3),
        }],
    };
    let Some(basis_result) = completed_query(request, &basis_query)? else {
        return Ok(DiscoveryResultV1::NeedsAccounts {
            format: DISCOVERY_RESULT_FORMAT_V1,
            addresses: Vec::new(),
            queries: vec![basis_query],
            minimum_finalized_slot: DecimalV1(snapshot_slot),
            require_clock: true,
            require_lp_position_rent: request.lp_position_rent_lamports.is_none(),
        });
    };
    let mut linked_basis = None;
    for address in &basis_result.addresses {
        let key = parse_key(address, "linked-basis query address")?;
        let candidate = corpus.observed(key, clock.unix_timestamp)?;
        if candidate.owner != registry
            || candidate.executable
            || candidate.data.get(..8)
                != Some(dclutch_product::payoff::runtime_v3::BASIS_MAGIC_V3.as_slice())
        {
            return Err(
                "Dealer linked-basis query result differs from its native filter".to_owned(),
            );
        }
        let Ok(decoded) = ProductBasisV3::decode(&candidate.data) else {
            continue;
        };
        if semantic_basis_id_v3(&candidate.data)
            .map_err(|_| "Dealer linked-basis semantic identity refused".to_owned())?
            != product_join.liability_basis_id.to_bytes()
        {
            continue;
        }
        if decoded.product_id() != product_join.product_id.to_bytes()
            || decoded.result_domain_id() != product_join.result_domain_id.to_bytes()
        {
            return Err("Dealer linked-basis Product composition refused".to_owned());
        }
        let digest = hash(&candidate.data).to_bytes();
        if candidate.key
            != record_address(
                registry,
                RAW_RECORD_PDA_SEED_V1,
                GRADED_BASIS_RECORD_SCHEMA_ID_V3,
                digest,
            )
        {
            return Err("Dealer linked-basis raw record address refused".to_owned());
        }
        if linked_basis.replace((candidate, digest)).is_some() {
            return Err("Dealer linked-basis observation is ambiguous".to_owned());
        }
    }
    let (linked_basis_account, linked_basis_digest) = linked_basis
        .ok_or_else(|| "Dealer linked-basis query found no selected semantic record".to_owned())?;
    let linked_basis_staging = record_address(
        registry,
        STAGING_CURSOR_PDA_SEED_V1,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        linked_basis_digest,
    );
    if let Some(result) = needs(&corpus, &[linked_basis_staging], request)? {
        return Ok(result);
    }
    require_record(
        &corpus,
        &linked_basis_account,
        registry,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        linked_basis_digest,
    )?;

    // The production topology projector consumes these authenticated base
    // observations and owns the remaining descriptor/artifact, child-route,
    // alias-packing, and admitted-caller rounds.  It is wired here in the same
    // convergence change that removes the fixture-owned implementation.
    let _ = observation;
    Err("Dealer production topology projection is not yet available".to_owned())
}

#[derive(Clone, Copy)]
struct ParsedProgramsV1 {
    core: Pubkey,
    registry: Pubkey,
    trading: Pubkey,
    claims: Pubkey,
    custody: Pubkey,
}

impl ParsedProgramsV1 {
    fn parse(value: &ProgramsV1) -> Result<Self, String> {
        let parsed = Self {
            core: parse_key(&value.core, "Core program")?,
            registry: parse_key(&value.registry, "Registry program")?,
            trading: parse_key(&value.trading, "Trading program")?,
            claims: parse_key(&value.claims, "Claims program")?,
            custody: parse_key(&value.custody, "Custody program")?,
        };
        let unique = BTreeSet::from(parsed.roles().map(|key| key.to_bytes()));
        if unique.len() != 5 || unique.contains(&[0; 32]) {
            return Err(
                "Dealer checked role programs must be five distinct nonzero keys".to_owned(),
            );
        }
        Ok(parsed)
    }

    fn roles(self) -> [Pubkey; 5] {
        [
            self.core,
            self.registry,
            self.trading,
            self.claims,
            self.custody,
        ]
    }
}

struct CorpusV1 {
    rows: Vec<(Pubkey, ObservedAccount)>,
}

impl CorpusV1 {
    fn parse(request: &DiscoveryRequestV1) -> Result<Self, String> {
        let slot = request.snapshot_slot.as_ref().map_or(0, |value| value.0);
        let observation = Observation {
            slot,
            unix_timestamp: 0,
            finality: Finality::Finalized,
        };
        let mut rows = Vec::with_capacity(request.accounts.len());
        for account in &request.accounts {
            if account.is_signer || account.is_writable {
                return Err(
                    "Dealer discovery corpus may not state instruction privileges".to_owned(),
                );
            }
            let key = parse_key(&account.address, "account address")?;
            if rows.iter().any(|(present, _)| *present == key) {
                return Err("Dealer discovery corpus contains a duplicate address".to_owned());
            }
            let owner = parse_key(&account.owner, "account owner")?;
            let data = STANDARD
                .decode(&account.data_base64)
                .map_err(|_| "Dealer account data is not base64".to_owned())?;
            if STANDARD.encode(&data) != account.data_base64 {
                return Err("Dealer account data is not canonical base64".to_owned());
            }
            rows.push((
                key,
                ObservedAccount {
                    observation,
                    key,
                    owner,
                    lamports: account.lamports.0,
                    executable: account.executable,
                    data,
                },
            ));
        }
        Ok(Self { rows })
    }

    fn contains(&self, key: Pubkey) -> bool {
        self.rows.iter().any(|(present, _)| *present == key)
    }

    fn observed(&self, key: Pubkey, unix_timestamp: i64) -> Result<ObservedAccount, String> {
        let mut account = self
            .rows
            .iter()
            .find(|(present, _)| *present == key)
            .map(|(_, account)| account.clone())
            .ok_or_else(|| format!("Dealer account corpus omits {key}"))?;
        account.observation.unix_timestamp = unix_timestamp;
        Ok(account)
    }
}

fn needs(
    corpus: &CorpusV1,
    addresses: &[Pubkey],
    request: &DiscoveryRequestV1,
) -> Result<Option<DiscoveryResultV1>, String> {
    let mut missing = Vec::new();
    for key in addresses {
        if !corpus.contains(*key) && !missing.contains(key) {
            missing.push(*key);
        }
    }
    let require_rent = request.lp_position_rent_lamports.is_none();
    if missing.is_empty() && !require_rent {
        return Ok(None);
    }
    Ok(Some(DiscoveryResultV1::NeedsAccounts {
        format: DISCOVERY_RESULT_FORMAT_V1,
        addresses: missing.into_iter().map(|key| key.to_string()).collect(),
        queries: Vec::new(),
        minimum_finalized_slot: DecimalV1(
            request.snapshot_slot.as_ref().map_or(0, |value| value.0),
        ),
        require_clock: true,
        require_lp_position_rent: require_rent,
    }))
}

fn completed_query<'a>(
    request: &'a DiscoveryRequestV1,
    query: &AccountQueryV1,
) -> Result<Option<&'a AccountQueryResultV1>, String> {
    let mut selected = None;
    for result in &request.query_results {
        if result.id != LINKED_BASIS_QUERY_ID_V1 {
            return Err("Dealer discovery contains an unknown query result".to_owned());
        }
        if result.id == query.id {
            if selected.replace(result).is_some() {
                return Err("Dealer discovery contains a duplicate query result".to_owned());
            }
            let mut addresses = BTreeSet::new();
            for address in &result.addresses {
                let key = parse_key(address, "query result address")?;
                if !addresses.insert(key.to_bytes()) {
                    return Err("Dealer discovery query result repeats an address".to_owned());
                }
                if !request
                    .accounts
                    .iter()
                    .any(|account| account.address == *address)
                {
                    return Err(
                        "Dealer discovery query result address lacks its account observation"
                            .to_owned(),
                    );
                }
            }
        }
    }
    Ok(selected)
}

fn parse_key(value: &str, label: &str) -> Result<Pubkey, String> {
    let key = Pubkey::from_str(value).map_err(|_| format!("Dealer {label} is not base58"))?;
    if key.to_string() != value {
        return Err(format!("Dealer {label} is not canonical base58"));
    }
    Ok(key)
}

fn decode_lower_hex_32(value: &str, label: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("Dealer {label} is not lowercase SHA-256 hex"));
    }
    let mut output = [0_u8; 32];
    for (index, slot) in output.iter_mut().enumerate() {
        let offset = index * 2;
        *slot = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| format!("Dealer {label} is not lowercase SHA-256 hex"))?;
    }
    if output == [0; 32] {
        return Err(format!("Dealer {label} is zero"));
    }
    Ok(output)
}

fn record_address(registry: Pubkey, domain: &[u8], schema: [u8; 32], digest: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[domain, &schema, &digest], &registry).0
}

fn push_record_pair(
    addresses: &mut Vec<Pubkey>,
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) {
    push_unique(
        addresses,
        record_address(registry, RAW_RECORD_PDA_SEED_V1, schema, digest),
    );
    push_unique(
        addresses,
        record_address(registry, STAGING_CURSOR_PDA_SEED_V1, schema, digest),
    );
}

fn push_unique(addresses: &mut Vec<Pubkey>, key: Pubkey) {
    if !addresses.contains(&key) {
        addresses.push(key);
    }
}

fn require_record(
    corpus: &CorpusV1,
    raw: &ObservedAccount,
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<(), String> {
    let staging = corpus.observed(
        record_address(registry, STAGING_CURSOR_PDA_SEED_V1, schema, digest),
        raw.observation.unix_timestamp,
    )?;
    if raw.key != record_address(registry, RAW_RECORD_PDA_SEED_V1, schema, digest)
        || raw.owner != registry
        || raw.executable
        || raw.lamports == 0
        || hash(&raw.data).to_bytes() != digest
        || staging.owner != system_program::ID
        || staging.lamports != 0
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err("Dealer finalized record or staging vacancy refused".to_owned());
    }
    Ok(())
}

fn protocol_position(
    claims_market: Pubkey,
    owner: Pubkey,
    claims_program: Pubkey,
) -> Result<Pubkey, String> {
    let seeds = ProtocolPositionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
        .map_err(|_| "Dealer Claims Position seeds refused".to_owned())?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &claims_program).0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DiscoveryRequestV1 {
        DiscoveryRequestV1 {
            format: DISCOVERY_INPUT_FORMAT_V1.to_owned(),
            intent: IntentV1 {
                market: Pubkey::new_from_array([1; 32]).to_string(),
                owner: Pubkey::new_from_array([2; 32]).to_string(),
                payer: Pubkey::new_from_array([3; 32]).to_string(),
                action: crate::wire::ActionV1::Open,
                expires_at: DecimalV1(9),
            },
            snapshot_slot: Some(DecimalV1(7)),
            programs: ProgramsV1 {
                core: Pubkey::new_from_array([4; 32]).to_string(),
                registry: Pubkey::new_from_array([5; 32]).to_string(),
                trading: Pubkey::new_from_array([6; 32]).to_string(),
                claims: Pubkey::new_from_array([7; 32]).to_string(),
                custody: Pubkey::new_from_array([8; 32]).to_string(),
            },
            checked_release: CheckedReleaseV1 {
                release_set: "09".repeat(32),
                trading_artifact_release: "0a".repeat(32),
                checked_manifest_digest: "0b".repeat(32),
                generation: DecimalV1(0),
            },
            accounts: Vec::new(),
            query_results: Vec::new(),
            lp_position_rent_lamports: Some(DecimalV1(1)),
        }
    }

    #[test]
    fn empty_corpus_requests_canonical_initial_addresses() {
        let request = request();
        let DiscoveryResultV1::NeedsAccounts {
            addresses,
            minimum_finalized_slot,
            ..
        } = discover(&request).expect("initial discovery")
        else {
            panic!("initial round must request accounts")
        };
        assert_eq!(minimum_finalized_slot, DecimalV1(7));
        assert_eq!(addresses.first(), Some(&request.intent.market));
        assert!(addresses.contains(&sysvar::clock::ID.to_string()));
        assert!(addresses.contains(&request.programs.trading));
    }

    #[test]
    fn substituted_privileges_and_stale_release_text_refuse_exactly() {
        let mut request = request();
        request.accounts.push(AccountV1 {
            address: request.intent.market.clone(),
            owner: request.programs.core.clone(),
            lamports: DecimalV1(1),
            executable: false,
            data_base64: String::new(),
            is_signer: false,
            is_writable: true,
        });
        assert_eq!(
            discover(&request).unwrap_err(),
            "Dealer discovery corpus may not state instruction privileges"
        );
        request.accounts.clear();
        request.checked_release.release_set = "AA".repeat(32);
        assert!(matches!(
            discover(&request),
            Ok(DiscoveryResultV1::NeedsAccounts { .. })
        ));
        assert_eq!(
            decode_lower_hex_32(&request.checked_release.release_set, "release set").unwrap_err(),
            "Dealer release set is not lowercase SHA-256 hex"
        );
    }
}
