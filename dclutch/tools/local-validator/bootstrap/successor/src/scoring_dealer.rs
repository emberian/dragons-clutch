//! The scoring Dealer's four routes, driven against an authenticated cluster.
//!
//! `DCLSFDR1`, `DCLSQTR1`, `DCLSFLR1` and `DCLSWDR1` are decision 0031's
//! second mechanism. Until this module they had a program, a codec, a Lean-
//! emitted frame table and no caller at all: not an operator, not a program
//! test, not a fixture. A route nothing can build an instruction for is a
//! design, not a mechanism.
//!
//! # What this reads and what it refuses to invent
//!
//! One `--market`, one `--dealer-id`, and the campaign report the founding
//! already wrote. Everything else is derived:
//!
//! * the Registry, the generation, the release set and the Realm digest come
//!   off the Core Market's own `CoreState`;
//! * the Trading, Claims, Custody and Core programs and their ProgramData come
//!   off the Registry activation cache the Market's release set selects;
//! * the collateral Mint and the token program come off the Realm record;
//! * the Claims aggregate is derived from the Market, and the Custody namespace
//!   -- the Hoard, the founding replay -- from the aggregate's own
//!   `custody_context`, which decision 0008 makes its sole persisted owner;
//! * the fund, the rule and the quote are the three Trading PDAs the records
//!   module derives, never flags;
//! * `claim_unit_atoms` and `K` come off the Market's linked-basis record,
//!   which is the same account `authenticate_claim_unit_v1` reads;
//! * a fill's `receive`, `deliver`, `prices` and `mint` come out of the
//!   host-only solver, which returns the fill the kernel already admitted.
//!
//! Only five addresses are taken from the campaign report rather than derived,
//! and each is a finalized Registry RECORD whose raw address this module
//! re-derives from the report's own `data_sha256` and refuses if it differs:
//! the Realm, the linked basis, the Product, the result domain and the
//! portfolio. Their content digests exist nowhere on chain that a caller can
//! reach, which is why the report is an input and not a convenience.
//!
//! # The agreement that has to hold exactly
//!
//! Every child window's coordinate zero is a `CallerAuthoritySeedsV1` PDA whose
//! last seed is the digest of the exact bytes the ROUTE will compose -- a
//! `CustodyRequestV1` for a Custody leg, the encoded signed-delta packet for
//! the Claims leg. A driver that composed those bytes differently by one field
//! would address a PDA nothing can sign. So this module composes the same
//! request structs the route composes, from the same chain state, and derives
//! the authority from the digest of those bytes.
//!
//! # Where the privileges come from, and why none of them are typed here
//!
//! Every prefix meta is the Lean-emitted privilege table read at its own
//! coordinate, and every window meta is the CHILD's frame spec read at its own
//! coordinate -- Custody's `CustodyFrameSpecV1`, Claims' `ClaimsFrameSpecV1`
//! and `SignedDeltaFrameSpecV3`. The route rebuilds the child metas from those
//! same specs (`child_metas_v1`) and refuses a window that holds less than the
//! spec declares, so what this module has to supply is exactly the spec's
//! writability -- never a guess, and never a number.
//!
//! The one privilege a caller cannot supply is a signature for a program
//! derived address. Coordinate zero of every window is the route's own caller
//! authority, and the route signs it with `invoke_signed`; at the top level it
//! is an ordinary read-only account, because a PDA has no key to sign with.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use solana_program::hash::hash;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
};
use solana_sdk_ids::{system_program, sysvar};

use dclutch_claims::CallerRole;
use dclutch_claims::frame_spec_v1::{ClaimsFrameRoleV1, ClaimsFrameSpecV1, SignedDeltaFrameSpecV3};
use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketSeedsV2,
    LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2, liability_basis_vector_width_v2,
};
use dclutch_claims::position_admission::USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1;
use dclutch_claims::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionActionV2,
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
};
use dclutch_claims::signed_delta_v3::{
    DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
    SignedDeltaPositionV3, SignedDeltaV3, ValidatedSignedDeltaConstructionV3,
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1,
    CustodyFrameRoleV1, CustodyFrameSpecV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyRequestV1, CustodyVaultSeedsV1, DelegatedCustodyRequestV2,
    INITIALIZE_REPLAY_ACCOUNT_COUNT_V1, OPEN_VAULT_ACCOUNT_COUNT_V1, OperationV1,
    TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_market::realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_market::{CoreState, Phase};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_product::payoff::runtime_v3::ProductBasisV3;
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::activation_auth_v1::activation_cache_address_v1;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::scoring_rule::generated;
use dclutch_trading::scoring_rule::records_v1::{DealerFundV1, DealerQuoteV1, ScoringRuleRecordV1};
use dclutch_trading::scoring_rule::requests_v1::{
    DealerFillRequestV1, DealerFoundRequestV1, DealerQuoteRequestV1, DealerReceiptV1,
    DealerRouteV1, DealerWithdrawRequestV1, fill_privileges_v1, found_privileges_v1,
    quote_privileges_v1, withdraw_privileges_v1,
};
use dclutch_trading::scoring_rule::solver::solve_buy;
use dclutch_trading::scoring_rule::{RuleParameters, Vector, potential, prices_of, subsidy_of};

use crate::campaign::{
    parse_campaign_terminal_evidence_with_expected_cluster_v1, read_keypair_file,
};
use crate::cluster::{ClusterOriginV1, ExpectedClusterV1};
use crate::model::TransactionEvidence;
use crate::plan::hex32;
use crate::rpc::{Rpc, RpcAccount, WritePolicyV1};
use crate::terminal_lifecycle::routed_record;
use crate::wallet_terminal::RecordPairV1;
use crate::{Error, Result};

/// Seal a rule, open a fund, admit the Dealer's Position, write the first quote.
pub(crate) const COMMAND_FOUND_V1: &str = "devnet-dealer-found-v1";
/// Write `p̂(inv)` into the Dealer's quote account, permissionlessly.
pub(crate) const COMMAND_QUOTE_V1: &str = "devnet-dealer-quote-v1";
/// A taker's fill against the Dealer: the batch of two.
pub(crate) const COMMAND_FILL_V1: &str = "devnet-dealer-fill-v1";
/// The sponsor takes cash out down to the floor `Φ`.
pub(crate) const COMMAND_WITHDRAW_V1: &str = "devnet-dealer-withdraw-v1";

/// The same planners and executor, restricted to an owned local validator.
pub(crate) const COMMAND_FOUND_LOCAL_V1: &str = "local-private-validator-dealer-found-v1";
pub(crate) const COMMAND_QUOTE_LOCAL_V1: &str = "local-private-validator-dealer-quote-v1";
pub(crate) const COMMAND_FILL_LOCAL_V1: &str = "local-private-validator-dealer-fill-v1";
pub(crate) const COMMAND_WITHDRAW_LOCAL_V1: &str = "local-private-validator-dealer-withdraw-v1";

/// The Claims signed-delta window: the fixed frame plus the two Positions.
const FILL_CLAIMS_WINDOW_ACCOUNTS: usize = 22;
/// The number of Custody `Transfer` legs a fill carries.
const FILL_CUSTODY_LEGS: usize = 3;

/// Which of the four the operator asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RouteV1 {
    Found,
    Quote,
    Fill,
    Withdraw,
}

impl RouteV1 {
    const fn command(self, expected: ExpectedClusterV1) -> &'static str {
        match (self, expected) {
            (Self::Found, ExpectedClusterV1::Devnet) => COMMAND_FOUND_V1,
            (Self::Quote, ExpectedClusterV1::Devnet) => COMMAND_QUOTE_V1,
            (Self::Fill, ExpectedClusterV1::Devnet) => COMMAND_FILL_V1,
            (Self::Withdraw, ExpectedClusterV1::Devnet) => COMMAND_WITHDRAW_V1,
            (Self::Found, ExpectedClusterV1::OwnedLoopback) => COMMAND_FOUND_LOCAL_V1,
            (Self::Quote, ExpectedClusterV1::OwnedLoopback) => COMMAND_QUOTE_LOCAL_V1,
            (Self::Fill, ExpectedClusterV1::OwnedLoopback) => COMMAND_FILL_LOCAL_V1,
            (Self::Withdraw, ExpectedClusterV1::OwnedLoopback) => COMMAND_WITHDRAW_LOCAL_V1,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Found => "scoring Dealer founding",
            Self::Quote => "scoring Dealer quote",
            Self::Fill => "scoring Dealer fill",
            Self::Withdraw => "scoring Dealer withdrawal",
        }
    }

    const fn receipt_route(self) -> DealerRouteV1 {
        match self {
            Self::Found => DealerRouteV1::Found,
            Self::Quote => DealerRouteV1::Quote,
            Self::Fill => DealerRouteV1::Fill,
            Self::Withdraw => DealerRouteV1::Withdraw,
        }
    }
}

pub(crate) fn usage() -> &'static str {
    "  dclutch-local-successor-bootstrap devnet-dealer-found-v1 --rpc-url URL --i-mean-devnet GENESIS --market PUBKEY --campaign-report ABSOLUTE_JSON --dealer-id HEX64 --sponsor PUBKEY --sponsor-token PUBKEY --liquidity CLAIM_UNITS --scale PRICE_DENOMINATOR --tolerance PRICE_UNITS --deposit ATOMS --evidence ABSOLUTE_JSON [--claim-unit-atoms ATOMS] [--execute --sponsor-keypair ABSOLUTE_JSON]\n\
     \n  dclutch-local-successor-bootstrap devnet-dealer-quote-v1 --rpc-url URL --i-mean-devnet GENESIS --market PUBKEY --campaign-report ABSOLUTE_JSON --dealer-id HEX64 --fee-payer PUBKEY --evidence ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     \n  dclutch-local-successor-bootstrap devnet-dealer-fill-v1 --rpc-url URL --i-mean-devnet GENESIS --market PUBKEY --campaign-report ABSOLUTE_JSON --dealer-id HEX64 --taker PUBKEY --taker-token PUBKEY --buy-outcome INDEX --buy-claims CLAIM_UNITS --evidence ABSOLUTE_JSON [--execute --taker-keypair ABSOLUTE_JSON]\n\
     \n  dclutch-local-successor-bootstrap devnet-dealer-withdraw-v1 --rpc-url URL --i-mean-devnet GENESIS --market PUBKEY --campaign-report ABSOLUTE_JSON --dealer-id HEX64 --sponsor PUBKEY --sponsor-token PUBKEY --amount ATOMS --evidence ABSOLUTE_JSON [--execute --sponsor-keypair ABSOLUTE_JSON]\n\
     \nOwned local-validator equivalents are local-private-validator-dealer-found-v1, local-private-validator-dealer-quote-v1, local-private-validator-dealer-fill-v1 and local-private-validator-dealer-withdraw-v1. They take the same route arguments with --rpc-url http://127.0.0.1:PORT and no --i-mean-devnet. Each arm authenticates both the RPC origin and the founding evidence against its selected cluster.\n\
     \nThe scoring Dealer (decision 0031 mechanism two), and the only caller of DCLSFDR1/DCLSQTR1/DCLSFLR1/DCLSWDR1 anywhere. Nothing economic is guessed: K and the claim-unit conversion come off the Market's own linked-basis record, the rule off the sealed rule account, the inventory off the Dealer's Claims Position, and a fill's receive/deliver/prices out of the host solver that returns only fills the kernel admits. Preflight opens no key and sends nothing. Execute sends one transaction and then reads the fund back and joins it to the route's own receipt shape."
}

/// Parsed command line, one shape for all four verbs.
#[derive(Debug)]
struct ArgumentsV1 {
    route: RouteV1,
    expected_cluster: ExpectedClusterV1,
    rpc_url: String,
    acknowledgment: Option<String>,
    market: Pubkey,
    campaign_report: PathBuf,
    dealer_id: [u8; 32],
    evidence: PathBuf,
    execute: bool,
    /// Founding and withdrawal: the sponsor of record.
    sponsor: Option<Pubkey>,
    sponsor_token: Option<Pubkey>,
    sponsor_keypair: Option<PathBuf>,
    /// Quote: any stranger.
    fee_payer: Option<Pubkey>,
    fee_payer_keypair: Option<PathBuf>,
    /// Fill: the taker.
    taker: Option<Pubkey>,
    taker_token: Option<Pubkey>,
    taker_keypair: Option<PathBuf>,
    buy_outcome: Option<u8>,
    buy_claims: Option<u64>,
    /// Founding: the rule the founding seals, and what it deposits.
    liquidity: Option<u64>,
    scale: Option<u64>,
    tolerance: Option<u64>,
    deposit: Option<u64>,
    claim_unit_atoms: Option<u64>,
    /// Withdrawal: atoms out.
    amount: Option<u64>,
}

pub(crate) fn run_found_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run(RouteV1::Found, ExpectedClusterV1::Devnet, arguments)
}

pub(crate) fn run_quote_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run(RouteV1::Quote, ExpectedClusterV1::Devnet, arguments)
}

pub(crate) fn run_fill_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run(RouteV1::Fill, ExpectedClusterV1::Devnet, arguments)
}

pub(crate) fn run_withdraw_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run(RouteV1::Withdraw, ExpectedClusterV1::Devnet, arguments)
}

pub(crate) fn run_found_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run(RouteV1::Found, ExpectedClusterV1::OwnedLoopback, arguments)
}

pub(crate) fn run_quote_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run(RouteV1::Quote, ExpectedClusterV1::OwnedLoopback, arguments)
}

pub(crate) fn run_fill_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run(RouteV1::Fill, ExpectedClusterV1::OwnedLoopback, arguments)
}

pub(crate) fn run_withdraw_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run(
        RouteV1::Withdraw,
        ExpectedClusterV1::OwnedLoopback,
        arguments,
    )
}

/// Authenticate before a socket, a founding report, or a key is opened.
fn authenticate_origin(arguments: &ArgumentsV1) -> Result<ClusterOriginV1> {
    if arguments.expected_cluster == ExpectedClusterV1::Devnet && arguments.acknowledgment.is_none()
    {
        return Err(Error::new(format!(
            "--i-mean-devnet GENESIS_HASH is required to reach a public cluster through {}",
            arguments.route.command(arguments.expected_cluster)
        )));
    }
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, arguments.acknowledgment.as_deref())?;
    arguments.expected_cluster.authenticate(&origin)?;
    Ok(origin)
}

/// Everything both clusters and all four routes share: origin, plan and evidence.
fn run(route: RouteV1, expected_cluster: ExpectedClusterV1, arguments: Vec<String>) -> Result<()> {
    let arguments = parse(route, expected_cluster, arguments)?;
    let origin = authenticate_origin(&arguments)?;
    // ReadsOnly on a preflight is what makes "nothing was sent" a property of
    // the transport rather than a promise this function makes.
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&origin, policy)?;
    let cluster = origin.label().to_owned();

    let coordinates = derive_coordinates(&mut rpc, &arguments)?;
    let plan = match route {
        RouteV1::Found => plan_found(&mut rpc, &arguments, &coordinates),
        RouteV1::Quote => plan_quote(&mut rpc, &arguments, &coordinates),
        RouteV1::Fill => plan_fill(&mut rpc, &arguments, &coordinates),
        RouteV1::Withdraw => plan_withdraw(&mut rpc, &arguments, &coordinates),
    }?;
    report(&coordinates, &plan);

    if !arguments.execute {
        write_evidence(
            &arguments.evidence,
            &coordinates,
            &plan,
            None,
            None,
            &cluster,
            expected_cluster,
        )?;
        println!("preflight only; no key was opened and nothing was sent");
        return Ok(());
    }

    let (path, expected) = match route {
        RouteV1::Found | RouteV1::Withdraw => (
            arguments.sponsor_keypair.as_deref(),
            arguments
                .sponsor
                .ok_or_else(|| Error::new("--sponsor is required"))?,
        ),
        RouteV1::Quote => (
            arguments.fee_payer_keypair.as_deref(),
            arguments
                .fee_payer
                .ok_or_else(|| Error::new("--fee-payer is required"))?,
        ),
        RouteV1::Fill => (
            arguments.taker_keypair.as_deref(),
            arguments
                .taker
                .ok_or_else(|| Error::new("--taker is required"))?,
        ),
    };
    let path = path.ok_or_else(|| {
        Error::new(format!(
            "--execute requires the keypair of the {} frame's one signer",
            route.label()
        ))
    })?;
    let signer = Keypair::new_from_array(read_keypair_file(path, route.label())?);
    if signer.pubkey() != expected {
        return Err(Error::new(format!(
            "the plan was built for {expected} and the keypair holds {}; the signer is a frame \
             coordinate, so it cannot be substituted after planning",
            signer.pubkey()
        )));
    }
    println!("signer               {}", signer.pubkey());

    // The frames that exceed the 1,232-byte legacy packet route their account
    // keys through one frozen table this run publishes; the two that fit do
    // not, and say so rather than paying for a table they do not need.
    let mut published = Vec::new();
    let landed = if plan.routed {
        let (observation, tables) = crate::market::publish_routing_table(
            &mut rpc,
            &signer,
            "DCLSDLR1",
            std::slice::from_ref(&plan.instruction),
            &mut published,
        )?;
        rpc.send_v0(
            route.label(),
            std::slice::from_ref(&plan.instruction),
            &signer,
            observation,
            &tables,
        )?
    } else {
        rpc.send(
            route.label(),
            std::slice::from_ref(&plan.instruction),
            &signer,
        )?
    };
    if let Some(error) = landed.error.as_ref() {
        return Err(Error::new(format!(
            "the {} refused on chain: {error}",
            route.label()
        )));
    }
    println!("signature            {}", landed.signature);
    println!("slot                 {}", landed.slot);
    println!(
        "compute units        {}",
        landed
            .compute_units_consumed
            .map_or_else(|| "unreported".to_string(), |units| units.to_string())
    );

    let poststate = read_back(&mut rpc, &arguments, &coordinates, &plan)?;
    write_evidence(
        &arguments.evidence,
        &coordinates,
        &plan,
        Some(signer.pubkey()),
        Some((&landed, &poststate)),
        &cluster,
        expected_cluster,
    )?;
    Ok(())
}

/// Everything one submission is, before any key exists.
struct PlanV1 {
    route: RouteV1,
    instruction: Instruction,
    /// The exact request bytes, kept so the receipt join can use them.
    request: Vec<u8>,
    /// Whether the frame needs an address lookup table to fit a packet.
    routed: bool,
    /// The three Trading PDAs.
    fund: Pubkey,
    rule: Pubkey,
    quote: Pubkey,
    /// The Dealer's Claims Position.
    dealer_position: Pubkey,
    /// The fund revision the request names.
    expected_fund_revision: u64,
    /// Every window authority, in frame order, for the evidence.
    authorities: Vec<(String, Pubkey)>,
    /// Route-specific scalars, already named.
    facts: Value,
    /// The Dealer's claim units in and out, zero outside a fill.
    dealer_pays: u64,
    dealer_receives: u64,
}

/// What the chain says about this Market before any route is planned.
struct CoordinatesV1 {
    market: Pubkey,
    generation: u64,
    phase: Phase,
    release_set: [u8; 32],
    realm_digest: [u8; 32],
    registry: Pubkey,
    activation_cache: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
    claims_program: Pubkey,
    claims_programdata: Pubkey,
    trading_program: Pubkey,
    trading_programdata: Pubkey,
    custody_program: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    custody_authority: Pubkey,
    /// The Claims aggregate and the namespace it owns.
    aggregate: Pubkey,
    custody_context: [u8; 32],
    hoard: Pubkey,
    hoard_replay: Pubkey,
    claim_count: u32,
    aggregate_revision: u64,
    basis_id: [u8; 32],
    /// The five finalized records the child frames name.
    realm: RecordPairV1,
    linked_basis: RecordPairV1,
    product: RecordPairV1,
    result_domain: RecordPairV1,
    portfolio: RecordPairV1,
    /// The Market's own claim-unit conversion and ordinary width.
    claim_unit_atoms: u64,
    outcome_count: u8,
    /// The Claims Position rent credit and the program that owns it.
    rent_credit: Pubkey,
    rent_program: Pubkey,
}

/// Read the Market, its release, its Realm, its aggregate and its basis record.
fn derive_coordinates(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<CoordinatesV1> {
    let market_account = rpc.required_account(arguments.market, "Core Market")?;
    let state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market {}: {error:?}", arguments.market)))?;
    let core_program = market_account.owner;
    let registry = Pubkey::new_from_array(state.identity.registry_program.to_bytes());
    let release_set = state.identity.selected_release_set.to_bytes();
    let realm_digest = state.identity.realm_id.to_bytes();

    let activation_cache = activation_cache_address_v1(&registry, &release_set);
    let cache = rpc.required_account(activation_cache, "Registry activation cache")?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache.data)
        .map_err(|error| Error::new(format!("activation cache: {error:?}")))?;
    let role = |role: ExecutionRoleV1, label: &str| -> Result<(Pubkey, Pubkey)> {
        let selected = activated
            .role(role)
            .map_err(|error| Error::new(format!("{label} role: {error:?}")))?;
        let release = selected.release();
        Ok((
            Pubkey::new_from_array(release.program().to_bytes()),
            Pubkey::new_from_array(release.programdata()),
        ))
    };
    let (core_selected, core_programdata) = role(ExecutionRoleV1::Core, "Core")?;
    let (claims_program, claims_programdata) = role(ExecutionRoleV1::Claims, "Claims")?;
    let (trading_program, trading_programdata) = role(ExecutionRoleV1::Trading, "Trading")?;
    let (custody_program, _) = role(ExecutionRoleV1::Custody, "Custody")?;
    if core_selected != core_program {
        return Err(Error::new(format!(
            "the Market at {} is owned by {core_program}, but its release selects Core \
             {core_selected}",
            arguments.market
        )));
    }

    let evidence_bytes = std::fs::read(&arguments.campaign_report)
        .map_err(|error| Error::new(format!("{}: {error}", arguments.campaign_report.display())))?;
    let evidence = parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &evidence_bytes,
        arguments.expected_cluster,
    )?;
    let realm = routed_record(
        &evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let linked_basis = routed_record(
        &evidence,
        "linked_liability_basis_record",
        registry,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    )?;
    let product = routed_record(
        &evidence,
        "product_record",
        registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
    )?;
    let result_domain = routed_record(
        &evidence,
        "result_domain_record",
        registry,
        RESULT_DOMAIN_SCHEMA_ID_V2,
    )?;
    let portfolio = routed_record(
        &evidence,
        "portfolio_record",
        registry,
        PORTFOLIO_SCHEMA_ID_V2,
    )?;
    let rent_credit = crate::plan::pubkey(
        &crate::terminal_lifecycle::required_account(&evidence, "founding_lifecycle_rent_credit")?
            .address,
    )?;
    let rent_credit_account = rpc.required_account(rent_credit, "lifecycle rent credit")?;
    let rent_program = rent_credit_account.owner;

    // The Realm the Market names is what selects the collateral atom; a driver
    // that took a Mint on the command line would be a second authority over
    // which token this Dealer trades.
    let realm_account = rpc.required_account(realm.raw, "Realm record")?;
    let realm_body = RealmV1::decode(&realm_account.data)
        .map_err(|error| Error::new(format!("Realm record {}: {error:?}", realm.raw)))?;
    let mint = Pubkey::new_from_array(*realm_body.collateral_mint());
    let token_program = Pubkey::new_from_array(*realm_body.token_program());

    let aggregate_seeds = LiabilityBasisMarketSeedsV2::new(arguments.market.to_bytes())
        .map_err(|error| Error::new(format!("Claims aggregate seeds: {error:?}")))?;
    let aggregate = Pubkey::find_program_address(&aggregate_seeds.as_slices(), &claims_program).0;
    let aggregate_account = rpc.required_account(aggregate, "Claims aggregate")?;
    if aggregate_account.owner != claims_program {
        return Err(Error::new(format!(
            "the Claims aggregate at {aggregate} is owned by {}, not the release's Claims program \
             {claims_program}",
            aggregate_account.owner
        )));
    }
    let view = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate {aggregate}: {error:?}")))?;
    if view.logical_market != arguments.market.to_bytes() || view.release_set != release_set {
        return Err(Error::new(
            "the Claims aggregate names another logical market or another release set",
        ));
    }

    // Decision 0008: the aggregate is the sole persisted owner of this Market's
    // Custody namespace. The Hoard and the founding replay are derived from it
    // and from nothing a caller can name.
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            arguments.market.to_bytes(),
            release_set,
            view.custody_context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &custody_program,
    )
    .0;
    let hoard_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            arguments.market.to_bytes(),
            release_set,
            CallerRoleV1::Trading,
            view.custody_context,
        )
        .as_slices(),
        &custody_program,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(arguments.market.to_bytes(), release_set).as_slices(),
        &custody_program,
    )
    .0;

    // `authenticate_claim_unit_v1` reads exactly these two facts off exactly
    // this record, so the driver reads them here rather than accepting them.
    let basis_account = rpc.required_account(linked_basis.raw, "linked basis record")?;
    let basis = ProductBasisV3::decode(&basis_account.data)
        .map_err(|error| Error::new(format!("linked basis record: {error:?}")))?;
    if basis.basis_width() != view.claim_count {
        return Err(Error::new(
            "the linked basis record's width is not the aggregate's claim count",
        ));
    }
    let ordinary = if basis.refunds_on_failure() {
        view.claim_count.checked_sub(1).ok_or_else(|| {
            Error::new("a refunding Market with no ordinary outcome hosts no Dealer")
        })?
    } else {
        view.claim_count
    };
    let outcome_count = u8::try_from(ordinary).map_err(|_| {
        Error::new(format!(
            "this Market's ordinary width {ordinary} exceeds a u8"
        ))
    })?;

    Ok(CoordinatesV1 {
        market: arguments.market,
        generation: state.identity.generation,
        phase: state.phase,
        release_set,
        realm_digest,
        registry,
        activation_cache,
        core_program,
        core_programdata,
        claims_program,
        claims_programdata,
        trading_program,
        trading_programdata,
        custody_program,
        mint,
        token_program,
        custody_authority,
        aggregate,
        custody_context: view.custody_context,
        hoard,
        hoard_replay,
        claim_count: view.claim_count,
        aggregate_revision: view.revision,
        basis_id: view.basis_id,
        realm,
        linked_basis,
        product,
        result_domain,
        portfolio,
        claim_unit_atoms: basis.payout_scale(),
        outcome_count,
        rent_credit,
        rent_program,
    })
}

impl CoordinatesV1 {
    /// The fund PDA for one Dealer under this Market.
    fn fund(&self, dealer_id: &[u8; 32]) -> Pubkey {
        Pubkey::find_program_address(
            &DealerFundV1::seeds(&self.market.to_bytes(), dealer_id),
            &self.trading_program,
        )
        .0
    }

    fn rule(&self, dealer_id: &[u8; 32]) -> Pubkey {
        Pubkey::find_program_address(
            &ScoringRuleRecordV1::seeds(&self.market.to_bytes(), dealer_id),
            &self.trading_program,
        )
        .0
    }

    fn quote(&self, dealer_id: &[u8; 32]) -> Pubkey {
        Pubkey::find_program_address(
            &DealerQuoteV1::seeds(&self.market.to_bytes(), dealer_id),
            &self.trading_program,
        )
        .0
    }

    /// One participant's canonical Claims Position under this aggregate.
    fn position(&self, owner: Pubkey) -> Result<Pubkey> {
        let seeds = ProtocolPositionSeedsV2::new(self.aggregate.to_bytes(), owner.to_bytes())
            .map_err(|error| Error::new(format!("Position seeds for {owner}: {error:?}")))?;
        Ok(Pubkey::find_program_address(&seeds.as_slices(), &self.claims_program).0)
    }

    /// The admission record paired with that Position.
    fn admission(&self, owner: Pubkey) -> Result<Pubkey> {
        let seeds =
            ProtocolPositionAdmissionSeedsV2::new(self.aggregate.to_bytes(), owner.to_bytes())
                .map_err(|error| Error::new(format!("admission seeds for {owner}: {error:?}")))?;
        Ok(Pubkey::find_program_address(&seeds.as_slices(), &self.claims_program).0)
    }

    /// The Custody replay for one context under the Trading role.
    fn replay(&self, context: [u8; 32]) -> Pubkey {
        Pubkey::find_program_address(
            &CustodyReplaySeedsV1::new(
                self.market.to_bytes(),
                self.release_set,
                CallerRoleV1::Trading,
                context,
            )
            .as_slices(),
            &self.custody_program,
        )
        .0
    }

    /// The fund's own `TradingPrincipal` vault: the vault context IS the fund.
    fn vault(&self, fund: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(
                self.market.to_bytes(),
                self.release_set,
                fund.to_bytes(),
                CompartmentV1::TradingPrincipal,
            )
            .as_slices(),
            &self.custody_program,
        )
        .0
    }

    /// The release-pinned Trading caller authority over one request digest.
    fn authority(&self, context: [u8; 32], request_digest: [u8; 32]) -> Result<Pubkey> {
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(self.release_set)
                .map_err(|error| Error::new(format!("release set: {error:?}")))?,
            self.market.to_bytes(),
            ExecutionRoleV1::Trading,
            context,
            request_digest,
        )
        .map_err(|error| Error::new(format!("caller authority seeds: {error:?}")))?;
        Ok(Pubkey::find_program_address(&seeds.as_slices(), &self.trading_program).0)
    }
}

/// One Custody leg's window coordinates and the authority over its request.
///
/// The request itself is not carried: its whole effect on the frame is the
/// digest the authority PDA is derived from, and keeping a second copy of it
/// here would be a second place a reader could believe.
struct CustodyLegPlanV1 {
    source: Pubkey,
    destination: Pubkey,
    /// `None` when the route skips the leg, which is what a zero amount means.
    authority: Option<Pubkey>,
    replay: Pubkey,
}

/// Compose one Custody `Transfer` request and derive the authority that signs it.
#[allow(clippy::too_many_arguments)]
fn custody_transfer_leg_v1(
    coordinates: &CoordinatesV1,
    context: [u8; 32],
    replay: Pubkey,
    expected_revision: u64,
    source: Pubkey,
    destination: Pubkey,
    source_compartment: CompartmentV1,
    destination_compartment: CompartmentV1,
    source_owner: [u8; 32],
    destination_owner: [u8; 32],
    source_vault_context: [u8; 32],
    destination_vault_context: [u8; 32],
    parent_request_digest: [u8; 32],
    transfer_index: u16,
    amount: u64,
) -> Result<CustodyLegPlanV1> {
    let request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment,
        destination_compartment,
        release_set: coordinates.release_set,
        market: coordinates.market.to_bytes(),
        realm: coordinates.realm_digest,
        context,
        caller_program: coordinates.trading_program.to_bytes(),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner,
            destination_owner,
            order: [0; 32],
            parent_request_digest,
            order_nonce: 0,
            generation: coordinates.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index,
        },
        source: source.to_bytes(),
        destination: destination.to_bytes(),
        source_vault_context,
        destination_vault_context,
        mint: coordinates.mint.to_bytes(),
        token_program: coordinates.token_program.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or_else(|| Error::new("Custody revision overflow"))?,
        amount,
        rent_lamports: 0,
    };
    // A zero leg is never invoked, so the route derives no authority for it and
    // there is no request to hash. The coordinate is filled with the System
    // program, which the route reaches past before it reads the window.
    let authority = if amount == 0 {
        None
    } else {
        let bytes = if source_compartment == CompartmentV1::External {
            DelegatedCustodyRequestV2 {
                custody: request,
                starts_atomic_debit: true,
                terminal: true,
                delegate_before: coordinates.custody_authority.to_bytes(),
                delegate_after: [0; 32],
                total_debit: amount,
                allowance_before: amount,
                allowance_after: 0,
            }
            .encode()
            .map_err(|error| Error::new(format!("delegated Custody transfer request: {error:?}")))?
            .to_vec()
        } else {
            request
                .to_bytes()
                .map_err(|error| Error::new(format!("Custody transfer request: {error:?}")))?
                .to_vec()
        };
        Some(coordinates.authority(context, hash(&bytes).to_bytes())?)
    };
    Ok(CustodyLegPlanV1 {
        source,
        destination,
        authority,
        replay,
    })
}

/// The fourteen metas of one Custody `Transfer` window, in the spec's order.
fn custody_transfer_window_v1(
    coordinates: &CoordinatesV1,
    leg: &CustodyLegPlanV1,
) -> Result<Vec<AccountMeta>> {
    let spec = CustodyFrameSpecV1::new(OperationV1::Transfer);
    custody_window_v1(spec, |role| match role {
        CustodyFrameRoleV1::CallerAuthority => Ok(leg.authority.unwrap_or(system_program::ID)),
        CustodyFrameRoleV1::Replay => Ok(leg.replay),
        CustodyFrameRoleV1::TransferSource => Ok(leg.source),
        CustodyFrameRoleV1::TransferDestination => Ok(leg.destination),
        other => coordinates.common_custody_key(other),
    })
}

impl CoordinatesV1 {
    /// The Custody coordinates every window shares.
    fn common_custody_key(&self, role: CustodyFrameRoleV1) -> Result<Pubkey> {
        match role {
            CustodyFrameRoleV1::CoreMarket => Ok(self.market),
            CustodyFrameRoleV1::ActivationCache => Ok(self.activation_cache),
            CustodyFrameRoleV1::RegistryProgram => Ok(self.registry),
            CustodyFrameRoleV1::CallerProgram => Ok(self.trading_program),
            CustodyFrameRoleV1::CallerProgramData => Ok(self.trading_programdata),
            CustodyFrameRoleV1::RealmRecord => Ok(self.realm.raw),
            CustodyFrameRoleV1::RealmStaging => Ok(self.realm.staging),
            CustodyFrameRoleV1::Mint => Ok(self.mint),
            CustodyFrameRoleV1::TokenProgram => Ok(self.token_program),
            CustodyFrameRoleV1::CustodyAuthority => Ok(self.custody_authority),
            CustodyFrameRoleV1::SystemProgram => Ok(system_program::ID),
            CustodyFrameRoleV1::RentSysvar => Ok(sysvar::rent::ID),
            other => Err(Error::new(format!(
                "the Custody coordinate {other:?} is not one this route can fill from the Market"
            ))),
        }
    }
}

/// Build one Custody window from its own frame spec, never from an index list.
fn custody_window_v1(
    spec: CustodyFrameSpecV1,
    key_of: impl Fn(CustodyFrameRoleV1) -> Result<Pubkey>,
) -> Result<Vec<AccountMeta>> {
    let mut metas = Vec::with_capacity(usize::from(spec.account_count()));
    for index in 0..spec.account_count() {
        let account = spec
            .account(index)
            .map_err(|error| Error::new(format!("Custody frame coordinate {index}: {error:?}")))?;
        let key = key_of(account.role())?;
        // The signer bit belongs to the CPI, never to the top level: the route
        // signs coordinate zero with its own PDA and `parse_prefix` refuses a
        // signer anywhere in a window.
        metas.push(if account.privileges().writable() {
            AccountMeta::new(key, false)
        } else {
            AccountMeta::new_readonly(key, false)
        });
    }
    Ok(metas)
}

/// Build one route's fixed prefix from the Lean-emitted privilege table.
///
/// A coordinate left unnamed is refused rather than compiled.
/// The plans fill a vector by named constant, so a frame that grows a
/// coordinate no plan writes would otherwise ship a placeholder at it and
/// refuse on chain with a code that names the frame and not the omission.
fn prefix_v1(
    keys: &[Option<Pubkey>],
    privileges: fn(usize) -> Option<(bool, bool)>,
) -> Result<Vec<AccountMeta>> {
    let mut metas = Vec::with_capacity(keys.len());
    for (index, key) in keys.iter().enumerate() {
        let (writable, signer) = privileges(index)
            .ok_or_else(|| Error::new(format!("the frame declares no coordinate {index}")))?;
        let key = (*key).ok_or_else(|| {
            Error::new(format!("this plan left frame coordinate {index} unnamed"))
        })?;
        metas.push(if writable {
            AccountMeta::new(key, signer)
        } else {
            AccountMeta::new_readonly(key, signer)
        });
    }
    if privileges(keys.len()).is_some() {
        return Err(Error::new(
            "the frame declares more coordinates than this plan named",
        ));
    }
    Ok(metas)
}

/// One decoded Custody replay, or the revision a vacant one starts at.
fn replay_revision(rpc: &mut Rpc, address: Pubkey, label: &str) -> Result<u64> {
    let account = rpc
        .account(address)?
        .ok_or_else(|| Error::new(format!("{label} {address} does not exist")))?;
    let replay = CustodyReplayV1::decode(&account.data)
        .map_err(|error| Error::new(format!("{label} {address}: {error:?}")))?;
    Ok(replay.next_revision)
}

fn optional_account(rpc: &mut Rpc, address: Pubkey) -> Result<Option<RpcAccount>> {
    rpc.account(address)
}

/// The Dealer's inventory, read the way the routes read it.
///
/// The owning PROGRAM is the release's Claims role and not the account's own
/// `owner` field: an account that satisfies a Position derivation under itself
/// is trivial to mint, and for a withdrawal a forged inventory raises `Φ` and
/// lets the sponsor take the subsidy with it.
fn read_inventory(
    rpc: &mut Rpc,
    coordinates: &CoordinatesV1,
    position: Pubkey,
    owner: Pubkey,
    width: u8,
) -> Result<Vector> {
    let account = rpc.required_account(position, "Claims Position")?;
    if account.owner != coordinates.claims_program {
        return Err(Error::new(format!(
            "the Position at {position} is owned by {}, not the release's Claims program {}",
            account.owner, coordinates.claims_program
        )));
    }
    let view = LiabilityBasisPositionViewV2::decode(&account.data)
        .map_err(|error| Error::new(format!("Claims Position {position}: {error:?}")))?;
    if view.owner != owner.to_bytes() {
        return Err(Error::new(format!(
            "the Position at {position} belongs to another owner, not {owner}"
        )));
    }
    if view.claim_count < u32::from(width) {
        return Err(Error::new(
            "the Position is narrower than the rule's outcome count",
        ));
    }
    let mut inventory = dclutch_trading::scoring_rule::ZERO_VECTOR;
    for (index, slot) in inventory.iter_mut().enumerate().take(usize::from(width)) {
        let claim = u32::try_from(index).map_err(|_| Error::new("outcome index overflow"))?;
        *slot = view
            .balance(&account.data, claim)
            .map_err(|error| Error::new(format!("Position balance {index}: {error:?}")))?;
    }
    Ok(inventory)
}

/// Read one fund and hold it to the Dealer this run names.
fn read_fund(
    rpc: &mut Rpc,
    fund: Pubkey,
    coordinates: &CoordinatesV1,
    dealer_id: [u8; 32],
) -> Result<(DealerFundV1, Vec<u8>)> {
    let account = rpc.required_account(fund, "Dealer fund")?;
    if account.owner != coordinates.trading_program {
        return Err(Error::new(format!(
            "the fund at {fund} is owned by {}, not the release's Trading program {}",
            account.owner, coordinates.trading_program
        )));
    }
    let value = DealerFundV1::decode(&account.data)
        .map_err(|error| Error::new(format!("Dealer fund {fund}: {error:?}")))?;
    if value.market != coordinates.market.to_bytes() || value.dealer_id != dealer_id {
        return Err(Error::new(
            "the fund at this address belongs to another Market or another Dealer",
        ));
    }
    Ok((value, account.data))
}

/// Read the sealed rule and hold it to the fund that carries its digest.
fn read_rule(rpc: &mut Rpc, rule: Pubkey, fund: DealerFundV1) -> Result<ScoringRuleRecordV1> {
    let account = rpc.required_account(rule, "sealed rule")?;
    if hash(&account.data).to_bytes() != fund.rule_digest {
        return Err(Error::new(format!(
            "the rule at {rule} is not the one the fund sealed"
        )));
    }
    ScoringRuleRecordV1::decode(&account.data)
        .map_err(|error| Error::new(format!("sealed rule {rule}: {error:?}")))
}

// ------------------------------------------------------------------- found

/// `DealerFound`: the Lean's 21-account frame, then four windows.
fn plan_found(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    coordinates: &CoordinatesV1,
) -> Result<PlanV1> {
    if coordinates.phase != Phase::Open {
        return Err(Error::new(
            "a Dealer is founded on an Open Market and this one is past it",
        ));
    }
    let sponsor = arguments
        .sponsor
        .ok_or_else(|| Error::new("--sponsor is required"))?;
    let sponsor_token = arguments
        .sponsor_token
        .ok_or_else(|| Error::new("--sponsor-token is required"))?;
    let deposit = arguments
        .deposit
        .ok_or_else(|| Error::new("--deposit is required"))?;
    let parameters = RuleParameters {
        outcome_count: coordinates.outcome_count,
        liquidity: arguments
            .liquidity
            .ok_or_else(|| Error::new("--liquidity is required"))?,
        scale: arguments
            .scale
            .ok_or_else(|| Error::new("--scale is required"))?,
        tolerance: arguments
            .tolerance
            .ok_or_else(|| Error::new("--tolerance is required"))?,
    }
    .admit()
    .map_err(|error| Error::new(format!("the rule's parameters are not admitted: {error:?}")))?;
    // The claim-unit conversion is the Market's, not the sponsor's. A stated
    // one is admitted only when it equals the record's own payout scale, which
    // is the exact conjunct `authenticate_claim_unit_v1` checks on chain.
    if let Some(stated) = arguments.claim_unit_atoms
        && stated != coordinates.claim_unit_atoms
    {
        return Err(Error::new(format!(
            "--claim-unit-atoms {stated} is not this Market's payout scale {}; the founding would \
             refuse as ClaimUnit",
            coordinates.claim_unit_atoms
        )));
    }
    let subsidy = subsidy_of(parameters)
        .map_err(|error| Error::new(format!("the rule's subsidy: {error:?}")))?;
    let deposit_claims = deposit / coordinates.claim_unit_atoms;
    if deposit_claims < subsidy {
        return Err(Error::new(format!(
            "the deposit is {deposit_claims} claim units and the rule's subsidy is {subsidy}; the \
             founding would refuse as Subsidy"
        )));
    }

    let fund = coordinates.fund(&arguments.dealer_id);
    let rule = coordinates.rule(&arguments.dealer_id);
    let quote = coordinates.quote(&arguments.dealer_id);
    let vault = coordinates.vault(fund);
    let replay = coordinates.replay(fund.to_bytes());
    let dealer_position = coordinates.position(fund)?;
    let dealer_admission = coordinates.admission(fund)?;

    let request = DealerFoundRequestV1 {
        parameters,
        market: coordinates.market.to_bytes(),
        dealer_id: arguments.dealer_id,
        release_set: coordinates.release_set,
        deposit,
        claim_unit_atoms: coordinates.claim_unit_atoms,
        generation: coordinates.generation,
    }
    .to_bytes()
    .map_err(|error| Error::new(format!("founding request: {error:?}")))?;
    let parent = hash(&request).to_bytes();

    // Claims first, because its caller authority is a PREFIX coordinate the
    // route binds to the Admit window's own coordinate zero.
    let admit = plan_admit_window_v1(
        rpc,
        coordinates,
        fund,
        dealer_position,
        dealer_admission,
        parent,
    )?;

    // Custody's three, at the only revisions it admits: 0 -> 1, 1 -> 2, 2 -> 3.
    let rent = rent_sysvar(rpc)?;
    let replay_request = CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: coordinates.release_set,
        market: coordinates.market.to_bytes(),
        realm: coordinates.realm_digest,
        context: fund.to_bytes(),
        caller_program: coordinates.trading_program.to_bytes(),
        semantic: founding_semantic_v1(coordinates, parent, 0),
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer: sponsor.to_bytes(),
        rent_refund: coordinates.rent_credit.to_bytes(),
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        rent_lamports: rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
    };
    let replay_authority = custody_authority_of(coordinates, replay_request, fund.to_bytes())?;
    let vault_request = CustodyRequestV1 {
        operation: OperationV1::OpenVault,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::TradingPrincipal,
        semantic: founding_semantic_v1(coordinates, parent, 1),
        destination: vault.to_bytes(),
        destination_vault_context: fund.to_bytes(),
        mint: coordinates.mint.to_bytes(),
        token_program: coordinates.token_program.to_bytes(),
        expected_revision: 1,
        resulting_revision: 2,
        rent_lamports: rent.minimum_balance(dclutch_custody::token_svm::ACCOUNT_BYTES),
        ..replay_request
    };
    let vault_authority = custody_authority_of(coordinates, vault_request, fund.to_bytes())?;
    let deposit_leg = custody_transfer_leg_v1(
        coordinates,
        fund.to_bytes(),
        replay,
        2,
        sponsor_token,
        vault,
        CompartmentV1::External,
        CompartmentV1::TradingPrincipal,
        sponsor.to_bytes(),
        [0; 32],
        [0; 32],
        fund.to_bytes(),
        parent,
        2,
        deposit,
    )?;

    let mut keys = vec![None; generated::FOUND_ACCOUNT_COUNT];
    keys[generated::FOUND_SPONSOR_ACCOUNT] = Some(sponsor);
    keys[generated::FOUND_FUND_ACCOUNT] = Some(fund);
    keys[generated::FOUND_RULE_ACCOUNT] = Some(rule);
    keys[generated::FOUND_QUOTE_ACCOUNT] = Some(quote);
    keys[generated::FOUND_MARKET_ACCOUNT] = Some(coordinates.market);
    keys[generated::FOUND_AGGREGATE_ACCOUNT] = Some(coordinates.aggregate);
    keys[generated::FOUND_DEALER_POSITION_ACCOUNT] = Some(dealer_position);
    keys[generated::FOUND_DEALER_ADMISSION_ACCOUNT] = Some(dealer_admission);
    keys[generated::FOUND_CLAIMS_AUTHORITY_ACCOUNT] = Some(admit.authority);
    keys[generated::FOUND_CLAIMS_PROGRAM_ACCOUNT] = Some(coordinates.claims_program);
    keys[generated::FOUND_SPONSOR_TOKEN_ACCOUNT] = Some(sponsor_token);
    keys[generated::FOUND_VAULT_ACCOUNT] = Some(vault);
    keys[generated::FOUND_CUSTODY_REPLAY_ACCOUNT] = Some(replay);
    keys[generated::FOUND_CUSTODY_AUTHORITY_ACCOUNT] = Some(coordinates.custody_authority);
    keys[generated::FOUND_CUSTODY_PROGRAM_ACCOUNT] = Some(coordinates.custody_program);
    keys[generated::FOUND_MINT_ACCOUNT] = Some(coordinates.mint);
    keys[generated::FOUND_TOKEN_PROGRAM_ACCOUNT] = Some(coordinates.token_program);
    keys[generated::FOUND_ACTIVATION_CACHE_ACCOUNT] = Some(coordinates.activation_cache);
    keys[generated::FOUND_REGISTRY_PROGRAM_ACCOUNT] = Some(coordinates.registry);
    keys[generated::FOUND_SYSTEM_PROGRAM_ACCOUNT] = Some(system_program::ID);
    keys[generated::FOUND_RENT_ACCOUNT] = Some(sysvar::rent::ID);
    let prefix = prefix_v1(&keys, found_privileges_v1)?;

    let mut windows = custody_window_v1(
        CustodyFrameSpecV1::new(OperationV1::InitializeReplay),
        |role| match role {
            CustodyFrameRoleV1::CallerAuthority => Ok(replay_authority),
            CustodyFrameRoleV1::Replay => Ok(replay),
            CustodyFrameRoleV1::Payer => Ok(sponsor),
            CustodyFrameRoleV1::RentRefund => Ok(coordinates.rent_credit),
            other => coordinates.common_custody_key(other),
        },
    )?;
    windows.extend(custody_window_v1(
        CustodyFrameSpecV1::new(OperationV1::OpenVault),
        |role| match role {
            CustodyFrameRoleV1::CallerAuthority => Ok(vault_authority),
            CustodyFrameRoleV1::Replay => Ok(replay),
            CustodyFrameRoleV1::Vault => Ok(vault),
            CustodyFrameRoleV1::Payer => Ok(sponsor),
            CustodyFrameRoleV1::RentRefund => Ok(coordinates.rent_credit),
            other => coordinates.common_custody_key(other),
        },
    )?);
    windows.extend(custody_transfer_window_v1(coordinates, &deposit_leg)?);
    windows.extend(admit.metas.iter().cloned());

    let expected_windows = usize::from(INITIALIZE_REPLAY_ACCOUNT_COUNT_V1)
        + usize::from(OPEN_VAULT_ACCOUNT_COUNT_V1)
        + usize::from(TRANSFER_ACCOUNT_COUNT_V1)
        + USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1;
    if windows.len() != expected_windows {
        return Err(Error::new(format!(
            "the founding's windows are {} accounts, not the route's {expected_windows}",
            windows.len()
        )));
    }

    let mut accounts = prefix;
    accounts.extend(windows);
    Ok(PlanV1 {
        route: RouteV1::Found,
        instruction: Instruction {
            program_id: coordinates.trading_program,
            accounts,
            data: request.to_vec(),
        },
        request: request.to_vec(),
        routed: true,
        fund,
        rule,
        quote,
        dealer_position,
        expected_fund_revision: 0,
        authorities: vec![
            ("custody-initialize-replay".to_string(), replay_authority),
            ("custody-open-vault".to_string(), vault_authority),
            (
                "custody-deposit".to_string(),
                deposit_leg.authority.unwrap_or(system_program::ID),
            ),
            ("claims-admit".to_string(), admit.authority),
        ],
        facts: json!({
            "outcomeCount": parameters.outcome_count,
            "liquidity": parameters.liquidity,
            "scale": parameters.scale,
            "tolerance": parameters.tolerance,
            "subsidyClaimUnits": subsidy,
            "depositAtoms": deposit,
            "depositClaimUnits": deposit_claims,
            "claimUnitAtoms": coordinates.claim_unit_atoms,
            "sponsor": sponsor.to_string(),
            "sponsorToken": sponsor_token.to_string(),
            "vault": vault.to_string(),
            "custodyReplay": replay.to_string(),
            "dealerAdmission": dealer_admission.to_string(),
        }),
        dealer_pays: 0,
        dealer_receives: 0,
    })
}

const fn founding_semantic_v1(
    coordinates: &CoordinatesV1,
    parent: [u8; 32],
    transfer_index: u16,
) -> ContextV1 {
    ContextV1 {
        candidate: [0; 32],
        source_owner: [0; 32],
        destination_owner: [0; 32],
        order: [0; 32],
        parent_request_digest: parent,
        order_nonce: 0,
        generation: coordinates.generation,
        page_index: 0,
        execution_index: 0,
        transfer_index,
    }
}

fn custody_authority_of(
    coordinates: &CoordinatesV1,
    request: CustodyRequestV1,
    context: [u8; 32],
) -> Result<Pubkey> {
    let bytes = request
        .to_bytes()
        .map_err(|error| Error::new(format!("Custody request: {error:?}")))?;
    coordinates.authority(context, hash(&bytes).to_bytes())
}

/// The Claims `Admit` window and the authority that signs its exact bytes.
struct AdmitWindowV1 {
    metas: Vec<AccountMeta>,
    authority: Pubkey,
}

fn plan_admit_window_v1(
    rpc: &mut Rpc,
    coordinates: &CoordinatesV1,
    fund: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    parent: [u8; 32],
) -> Result<AdmitWindowV1> {
    let rent = rent_sysvar(rpc)?;
    let position_account = optional_account(rpc, position)?;
    let admission_account = optional_account(rpc, admission)?;
    let observed =
        |account: &Option<RpcAccount>| account.as_ref().map_or(0, |value| value.lamports);
    let rent_plan = plan_admit_rents_v1(
        &rent,
        coordinates.claim_count,
        observed(&position_account),
        observed(&admission_account),
    )?;

    let claims_request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: coordinates.release_set,
        market: coordinates.market.to_bytes(),
        position_owner: fund.to_bytes(),
        parent_request_digest: parent,
        rent_credit: coordinates.rent_credit.to_bytes(),
        rent_program: coordinates.rent_program.to_bytes(),
        generation: coordinates.generation,
        expected_market_revision: coordinates.aggregate_revision,
        expected_position_revision: 0,
        observed_position_lamports: rent_plan.position_lamports,
        observed_admission_lamports: rent_plan.admission_lamports,
        position_rent_principal: rent_plan.position_principal,
        admission_rent_principal: rent_plan.admission_principal,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
    .new()
    .map_err(|error| Error::new(format!("Claims Position admission request: {error:?}")))?;
    // DealerFound owns a TradingRecord, not a wallet.  The user-lifecycle
    // outer intentionally rejects that owner kind, so derive this PDA from
    // the canonical Claims child wire that the route itself will invoke.
    let child = claims_request
        .to_bytes()
        .map_err(|error| Error::new(format!("Claims admission child bytes: {error:?}")))?;
    let authority = coordinates.authority(fund.to_bytes(), hash(&child).to_bytes())?;

    let spec = ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Admit);
    let width = spec
        .account_count()
        .map_err(|error| Error::new(format!("Claims Admit frame width: {error:?}")))?;
    let mut metas = Vec::with_capacity(usize::from(width));
    for index in 0..width {
        let account = spec
            .account(index)
            .map_err(|error| Error::new(format!("Claims Admit coordinate {index}: {error:?}")))?;
        let key = match account.role() {
            ClaimsFrameRoleV1::CallerAuthority => authority,
            ClaimsFrameRoleV1::ProtocolPosition => position,
            ClaimsFrameRoleV1::ProtocolPositionAdmission => admission,
            ClaimsFrameRoleV1::PositionOwnerIdentity => fund,
            other => coordinates.common_claims_key(other)?,
        };
        metas.push(if account.privileges().writable() {
            AccountMeta::new(key, false)
        } else {
            AccountMeta::new_readonly(key, false)
        });
    }
    Ok(AdmitWindowV1 { metas, authority })
}

/// The two CPI-time rent facts for a Dealer Position and admission record.
///
/// The PDAs are zero-length before the CPI, but DealerFound pre-funds the
/// canonical layouts before Claims allocates them. These must consequently be
/// the exact post-admission widths and balances rather than the observed
/// vacant widths and balances.
struct AdmitRentPlanV1 {
    position_principal: u64,
    admission_principal: u64,
    position_lamports: u64,
    admission_lamports: u64,
}

fn plan_admit_rents_v1(
    rent: &solana_sdk::rent::Rent,
    claim_count: u32,
    observed_position_lamports: u64,
    observed_admission_lamports: u64,
) -> Result<AdmitRentPlanV1> {
    let position_bytes =
        liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, claim_count)
            .map_err(|error| Error::new(format!("Claims Position width: {error:?}")))?;
    let position_principal = rent.minimum_balance(position_bytes);
    let admission_principal = rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    Ok(AdmitRentPlanV1 {
        position_principal,
        admission_principal,
        position_lamports: observed_position_lamports.max(position_principal),
        admission_lamports: observed_admission_lamports.max(admission_principal),
    })
}

impl CoordinatesV1 {
    /// The Claims coordinates both windows share.
    fn common_claims_key(&self, role: ClaimsFrameRoleV1) -> Result<Pubkey> {
        match role {
            ClaimsFrameRoleV1::ClaimsMarket => Ok(self.aggregate),
            ClaimsFrameRoleV1::BasisRecord => Ok(self.linked_basis.raw),
            ClaimsFrameRoleV1::BasisStaging => Ok(self.linked_basis.staging),
            ClaimsFrameRoleV1::ProductRecord => Ok(self.product.raw),
            ClaimsFrameRoleV1::ProductStaging => Ok(self.product.staging),
            ClaimsFrameRoleV1::ResultDomainRecord => Ok(self.result_domain.raw),
            ClaimsFrameRoleV1::ResultDomainStaging => Ok(self.result_domain.staging),
            ClaimsFrameRoleV1::PortfolioRecord => Ok(self.portfolio.raw),
            ClaimsFrameRoleV1::PortfolioStaging => Ok(self.portfolio.staging),
            ClaimsFrameRoleV1::RentSysvar => Ok(sysvar::rent::ID),
            ClaimsFrameRoleV1::SystemProgram => Ok(system_program::ID),
            ClaimsFrameRoleV1::CoreMarket => Ok(self.market),
            ClaimsFrameRoleV1::ActivationCache => Ok(self.activation_cache),
            ClaimsFrameRoleV1::RegistryProgram => Ok(self.registry),
            ClaimsFrameRoleV1::TradingProgram | ClaimsFrameRoleV1::CallerProgram => {
                Ok(self.trading_program)
            }
            ClaimsFrameRoleV1::TradingProgramData | ClaimsFrameRoleV1::CallerProgramData => {
                Ok(self.trading_programdata)
            }
            ClaimsFrameRoleV1::ClaimsProgram => Ok(self.claims_program),
            ClaimsFrameRoleV1::ClaimsProgramData => Ok(self.claims_programdata),
            ClaimsFrameRoleV1::CoreProgram => Ok(self.core_program),
            ClaimsFrameRoleV1::CoreProgramData => Ok(self.core_programdata),
            ClaimsFrameRoleV1::RentCredit => Ok(self.rent_credit),
            ClaimsFrameRoleV1::RentProgram => Ok(self.rent_program),
            other => Err(Error::new(format!(
                "the Claims coordinate {other:?} is not one this route can fill from the Market"
            ))),
        }
    }
}

fn rent_sysvar(rpc: &mut Rpc) -> Result<solana_sdk::rent::Rent> {
    let account = rpc.required_account(sysvar::rent::ID, "Rent sysvar")?;
    bincode::deserialize(&account.data).map_err(|error| Error::new(format!("Rent sysvar: {error}")))
}

// ------------------------------------------------------------------- quote

/// `DealerQuote`: the Lean's `quoteFrame` and no child window. Permissionless.
///
/// The frame carries the activation cache and the Registry because the route
/// now authenticates the release and holds the Position's owner to the
/// release's Claims role: a Position derived under the owner the ACCOUNT
/// claims proves nothing, since any program can mint an account that satisfies
/// a derivation under itself.
fn plan_quote(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    coordinates: &CoordinatesV1,
) -> Result<PlanV1> {
    let payer = arguments
        .fee_payer
        .ok_or_else(|| Error::new("--fee-payer is required"))?;
    let fund_key = coordinates.fund(&arguments.dealer_id);
    let (fund, _) = read_fund(rpc, fund_key, coordinates, arguments.dealer_id)?;
    let rule_key = coordinates.rule(&arguments.dealer_id);
    let rule = read_rule(rpc, rule_key, fund)?;
    let quote = coordinates.quote(&arguments.dealer_id);
    let dealer_position = coordinates.position(fund_key)?;
    let inventory = read_inventory(
        rpc,
        coordinates,
        dealer_position,
        fund_key,
        fund.outcome_count,
    )?;
    let prices = prices_of(rule.parameters, &inventory)
        .map_err(|error| Error::new(format!("the schedule at this inventory: {error:?}")))?;

    let request = DealerQuoteRequestV1 {
        market: coordinates.market.to_bytes(),
        dealer_id: arguments.dealer_id,
        expected_fund_revision: fund.revision,
    }
    .to_bytes()
    .map_err(|error| Error::new(format!("quote request: {error:?}")))?;

    let mut keys = vec![None; generated::QUOTE_ACCOUNT_COUNT];
    keys[generated::QUOTE_PAYER_ACCOUNT] = Some(payer);
    keys[generated::QUOTE_FUND_ACCOUNT] = Some(fund_key);
    keys[generated::QUOTE_RULE_ACCOUNT] = Some(rule_key);
    keys[generated::QUOTE_QUOTE_ACCOUNT] = Some(quote);
    keys[generated::QUOTE_MARKET_ACCOUNT] = Some(coordinates.market);
    keys[generated::QUOTE_DEALER_POSITION_ACCOUNT] = Some(dealer_position);
    keys[generated::QUOTE_ACTIVATION_CACHE_ACCOUNT] = Some(coordinates.activation_cache);
    keys[generated::QUOTE_REGISTRY_PROGRAM_ACCOUNT] = Some(coordinates.registry);

    Ok(PlanV1 {
        route: RouteV1::Quote,
        instruction: Instruction {
            program_id: coordinates.trading_program,
            accounts: prefix_v1(&keys, quote_privileges_v1)?,
            data: request.to_vec(),
        },
        request: request.to_vec(),
        routed: false,
        fund: fund_key,
        rule: rule_key,
        quote,
        dealer_position,
        expected_fund_revision: fund.revision,
        authorities: Vec::new(),
        facts: json!({
            "payer": payer.to_string(),
            "inventory": inventory[..usize::from(fund.outcome_count)].to_vec(),
            "prices": prices[..usize::from(fund.outcome_count)].to_vec(),
            "scale": rule.parameters.scale,
        }),
        dealer_pays: 0,
        dealer_receives: 0,
    })
}

// -------------------------------------------------------------------- fill

/// `DealerFill`: the Lean's 19-account frame, one Claims window, three Custody.
fn plan_fill(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    coordinates: &CoordinatesV1,
) -> Result<PlanV1> {
    if coordinates.phase != Phase::Open {
        return Err(Error::new(
            "a fill is admitted on an Open Market and this one is past it",
        ));
    }
    let taker = arguments
        .taker
        .ok_or_else(|| Error::new("--taker is required"))?;
    let taker_token = arguments
        .taker_token
        .ok_or_else(|| Error::new("--taker-token is required"))?;
    let outcome = arguments
        .buy_outcome
        .ok_or_else(|| Error::new("--buy-outcome is required"))?;
    let claims = arguments
        .buy_claims
        .ok_or_else(|| Error::new("--buy-claims is required"))?;

    let fund_key = coordinates.fund(&arguments.dealer_id);
    let (fund, _) = read_fund(rpc, fund_key, coordinates, arguments.dealer_id)?;
    let rule_key = coordinates.rule(&arguments.dealer_id);
    let rule = read_rule(rpc, rule_key, fund)?;
    let dealer_position = coordinates.position(fund_key)?;
    let taker_position = coordinates.position(taker)?;
    let inventory = read_inventory(
        rpc,
        coordinates,
        dealer_position,
        fund_key,
        fund.outcome_count,
    )?;

    // The solver, not this driver, decides what moves: it walks the lattice and
    // returns the first point `admit_fill` already accepted, carrying the exact
    // `p̂(inv′)` R2 will re-derive.
    let solved = solve_buy(rule.parameters, &inventory, usize::from(outcome), claims)
        .map_err(|error| Error::new(format!("the solver found no admitted fill: {error:?}")))?;

    let request_body = DealerFillRequestV1 {
        outcome_count: fund.outcome_count,
        market: coordinates.market.to_bytes(),
        dealer_id: arguments.dealer_id,
        taker: taker.to_bytes(),
        expected_fund_revision: fund.revision,
        mint: solved.mint,
        prices: solved.prices,
        receive: solved.receive,
        deliver: solved.deliver,
    };
    let request = request_body
        .to_bytes()
        .map_err(|error| Error::new(format!("fill request: {error:?}")))?;
    let parent = hash(&request).to_bytes();

    // The cash, atoms, and the legs `cashLegs` derives from them.
    let (pays_atoms, receives_atoms) = fund
        .debit_atoms(solved.admitted.dealer_pays, solved.admitted.dealer_receives)
        .map_err(|error| Error::new(format!("the Dealer's debit in atoms: {error:?}")))?;
    let mint_atoms = fund
        .atoms(solved.mint)
        .map_err(|error| Error::new(format!("the mint's par in atoms: {error:?}")))?;
    let legs = cash_legs_v1(mint_atoms, pays_atoms, receives_atoms);
    if legs.fund_to_hoard.saturating_add(legs.fund_to_taker) > fund.cash {
        return Err(Error::new(format!(
            "the fill would move {} atoms out of a fund holding {}; it would refuse as Uncovered",
            legs.fund_to_hoard.saturating_add(legs.fund_to_taker),
            fund.cash
        )));
    }

    let vault = coordinates.vault(fund_key);
    let fund_replay = coordinates.replay(fund_key.to_bytes());
    let fund_revision = replay_revision(rpc, fund_replay, "the fund's Custody replay")?;
    let hoard_revision =
        replay_revision(rpc, coordinates.hoard_replay, "the Hoard's Custody replay")?;
    // Leg A advances the fund's replay only when it is actually invoked, and
    // leg C re-reads the cursor afterwards. A driver that assumed the advance
    // would name a revision Custody has not reached.
    let leg_c_revision = if legs.fund_to_hoard > 0 {
        fund_revision
            .checked_add(1)
            .ok_or_else(|| Error::new("Custody revision overflow"))?
    } else {
        fund_revision
    };
    let leg_a = custody_transfer_leg_v1(
        coordinates,
        fund_key.to_bytes(),
        fund_replay,
        fund_revision,
        vault,
        coordinates.hoard,
        CompartmentV1::TradingPrincipal,
        CompartmentV1::HoardPrincipal,
        [0; 32],
        [0; 32],
        fund_key.to_bytes(),
        coordinates.custody_context,
        parent,
        0,
        legs.fund_to_hoard,
    )?;
    let leg_b = custody_transfer_leg_v1(
        coordinates,
        coordinates.custody_context,
        coordinates.hoard_replay,
        hoard_revision,
        taker_token,
        coordinates.hoard,
        CompartmentV1::External,
        CompartmentV1::HoardPrincipal,
        taker.to_bytes(),
        [0; 32],
        [0; 32],
        coordinates.custody_context,
        parent,
        1,
        legs.taker_to_hoard,
    )?;
    let to_taker = legs.fund_to_taker > 0;
    let leg_c = custody_transfer_leg_v1(
        coordinates,
        fund_key.to_bytes(),
        fund_replay,
        leg_c_revision,
        if to_taker { vault } else { taker_token },
        if to_taker { taker_token } else { vault },
        if to_taker {
            CompartmentV1::TradingPrincipal
        } else {
            CompartmentV1::External
        },
        if to_taker {
            CompartmentV1::External
        } else {
            CompartmentV1::TradingPrincipal
        },
        if to_taker { [0; 32] } else { taker.to_bytes() },
        if to_taker { taker.to_bytes() } else { [0; 32] },
        if to_taker {
            fund_key.to_bytes()
        } else {
            [0; 32]
        },
        if to_taker {
            [0; 32]
        } else {
            fund_key.to_bytes()
        },
        parent,
        2,
        legs.fund_to_taker.max(legs.taker_to_fund),
    )?;

    let claims_window = plan_signed_delta_window_v1(
        rpc,
        coordinates,
        fund,
        fund_key,
        taker,
        dealer_position,
        taker_position,
        request_body,
        parent,
    )?;

    let mut keys = vec![None; generated::FILL_ACCOUNT_COUNT];
    keys[generated::FILL_TAKER_ACCOUNT] = Some(taker);
    keys[generated::FILL_FUND_ACCOUNT] = Some(fund_key);
    keys[generated::FILL_RULE_ACCOUNT] = Some(rule_key);
    keys[generated::FILL_MARKET_ACCOUNT] = Some(coordinates.market);
    keys[generated::FILL_AGGREGATE_ACCOUNT] = Some(coordinates.aggregate);
    keys[generated::FILL_DEALER_POSITION_ACCOUNT] = Some(dealer_position);
    keys[generated::FILL_TAKER_POSITION_ACCOUNT] = Some(taker_position);
    keys[generated::FILL_CLAIMS_AUTHORITY_ACCOUNT] = Some(claims_window.authority);
    keys[generated::FILL_CLAIMS_PROGRAM_ACCOUNT] = Some(coordinates.claims_program);
    keys[generated::FILL_TAKER_TOKEN_ACCOUNT] = Some(taker_token);
    keys[generated::FILL_VAULT_ACCOUNT] = Some(vault);
    keys[generated::FILL_HOARD_ACCOUNT] = Some(coordinates.hoard);
    keys[generated::FILL_CUSTODY_REPLAY_ACCOUNT] = Some(fund_replay);
    keys[generated::FILL_CUSTODY_AUTHORITY_ACCOUNT] = Some(coordinates.custody_authority);
    keys[generated::FILL_CUSTODY_PROGRAM_ACCOUNT] = Some(coordinates.custody_program);
    keys[generated::FILL_MINT_ACCOUNT] = Some(coordinates.mint);
    keys[generated::FILL_TOKEN_PROGRAM_ACCOUNT] = Some(coordinates.token_program);
    keys[generated::FILL_ACTIVATION_CACHE_ACCOUNT] = Some(coordinates.activation_cache);
    keys[generated::FILL_REGISTRY_PROGRAM_ACCOUNT] = Some(coordinates.registry);
    let prefix = prefix_v1(&keys, fill_privileges_v1)?;

    let mut windows = claims_window.metas;
    for leg in [&leg_a, &leg_b, &leg_c] {
        windows.extend(custody_transfer_window_v1(coordinates, leg)?);
    }
    let expected_windows =
        FILL_CLAIMS_WINDOW_ACCOUNTS + FILL_CUSTODY_LEGS * usize::from(TRANSFER_ACCOUNT_COUNT_V1);
    if windows.len() != expected_windows {
        return Err(Error::new(format!(
            "the fill's windows are {} accounts, not the route's {expected_windows}",
            windows.len()
        )));
    }

    let mut accounts = prefix;
    accounts.extend(windows);
    Ok(PlanV1 {
        route: RouteV1::Fill,
        instruction: Instruction {
            program_id: coordinates.trading_program,
            accounts,
            data: request.to_vec(),
        },
        request: request.to_vec(),
        routed: true,
        fund: fund_key,
        rule: rule_key,
        quote: coordinates.quote(&arguments.dealer_id),
        dealer_position,
        expected_fund_revision: fund.revision,
        authorities: vec![
            ("claims-signed-delta".to_string(), claims_window.authority),
            (
                "custody-fund-to-hoard".to_string(),
                leg_a.authority.unwrap_or(system_program::ID),
            ),
            (
                "custody-taker-to-hoard".to_string(),
                leg_b.authority.unwrap_or(system_program::ID),
            ),
            (
                "custody-net".to_string(),
                leg_c.authority.unwrap_or(system_program::ID),
            ),
        ],
        facts: json!({
            "taker": taker.to_string(),
            "takerToken": taker_token.to_string(),
            "takerPosition": taker_position.to_string(),
            "buyOutcome": outcome,
            "buyClaimUnits": claims,
            "mintClaimUnits": solved.mint,
            "mintAtoms": mint_atoms,
            "receive": solved.receive[..usize::from(fund.outcome_count)].to_vec(),
            "deliver": solved.deliver[..usize::from(fund.outcome_count)].to_vec(),
            "prices": solved.prices[..usize::from(fund.outcome_count)].to_vec(),
            "dealerPaysClaimUnits": solved.admitted.dealer_pays,
            "dealerReceivesClaimUnits": solved.admitted.dealer_receives,
            "dealerPaysAtoms": pays_atoms,
            "dealerReceivesAtoms": receives_atoms,
            "legFundToHoard": legs.fund_to_hoard,
            "legTakerToHoard": legs.taker_to_hoard,
            "legFundToTaker": legs.fund_to_taker,
            "legTakerToFund": legs.taker_to_fund,
            "hoard": coordinates.hoard.to_string(),
            "vault": vault.to_string(),
        }),
        dealer_pays: solved.admitted.dealer_pays,
        dealer_receives: solved.admitted.dealer_receives,
    })
}

/// The cash legs, atoms.
///
/// `ScoringRuleAbiV1.cashLegs`, restated because the route that owns it lives
/// behind the Trading program's `dealer-family` feature and this binary does
/// not link it. The two invariants the Lean names -- the Hoard receives exactly
/// the mint's par, and the fund moves by exactly the Dealer's debit -- are
/// pinned by this module's own tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CashLegsV1 {
    taker_to_hoard: u64,
    fund_to_hoard: u64,
    fund_to_taker: u64,
    taker_to_fund: u64,
}

fn cash_legs_v1(mint_atoms: u64, dealer_pays_atoms: u64, dealer_receives_atoms: u64) -> CashLegsV1 {
    if dealer_receives_atoms == 0 {
        let fund_to_hoard = dealer_pays_atoms.min(mint_atoms);
        CashLegsV1 {
            taker_to_hoard: mint_atoms - fund_to_hoard,
            fund_to_hoard,
            fund_to_taker: dealer_pays_atoms - fund_to_hoard,
            taker_to_fund: 0,
        }
    } else {
        CashLegsV1 {
            taker_to_hoard: mint_atoms,
            fund_to_hoard: 0,
            fund_to_taker: 0,
            taker_to_fund: dealer_receives_atoms,
        }
    }
}

/// The Claims signed-delta window and the authority over its exact packet.
struct SignedDeltaWindowV1 {
    metas: Vec<AccountMeta>,
    authority: Pubkey,
}

#[allow(clippy::too_many_arguments)]
fn plan_signed_delta_window_v1(
    rpc: &mut Rpc,
    coordinates: &CoordinatesV1,
    fund: DealerFundV1,
    fund_key: Pubkey,
    taker: Pubkey,
    dealer_position: Pubkey,
    taker_position: Pubkey,
    request: DealerFillRequestV1,
    parent: [u8; 32],
) -> Result<SignedDeltaWindowV1> {
    let revision_of = |rpc: &mut Rpc, address: Pubkey, owner: Pubkey| -> Result<u64> {
        let account = rpc.required_account(address, "Claims Position")?;
        let view = LiabilityBasisPositionViewV2::decode(&account.data)
            .map_err(|error| Error::new(format!("Claims Position {address}: {error:?}")))?;
        if view.owner != owner.to_bytes() || view.market_account != coordinates.aggregate.to_bytes()
        {
            return Err(Error::new(format!(
                "the Position at {address} is not {owner}'s under this Market's aggregate"
            )));
        }
        Ok(view.revision)
    };
    let dealer_revision = revision_of(rpc, dealer_position, fund_key)?;
    let taker_revision = revision_of(rpc, taker_position, taker)?;

    // The two record digests the route hashes off the window's own accounts.
    let digest_of = |rpc: &mut Rpc, address: Pubkey, label: &str| -> Result<[u8; 32]> {
        let account = rpc.required_account(address, label)?;
        Ok(hash(&account.data).to_bytes())
    };
    let product_record_digest = digest_of(rpc, coordinates.product.raw, "Product record")?;
    let linked_basis_record_digest =
        digest_of(rpc, coordinates.linked_basis.raw, "linked basis record")?;

    let input = SignedDeltaPlanInputV3 {
        caller_role: CallerRole::Trading,
        release_set: coordinates.release_set,
        market: coordinates.market.to_bytes(),
        request_id: parent,
        product_record_digest,
        semantic_basis_id: coordinates.basis_id,
        linked_basis_record_digest,
        expected_market_revision: coordinates.aggregate_revision,
        claim_count: coordinates.claim_count,
    };
    let dealer_position_entry = SignedDeltaPositionV3::new(fund_key.to_bytes(), dealer_revision)
        .map_err(|error| Error::new(format!("the Dealer's delta position: {error:?}")))?;
    let taker_position_entry = SignedDeltaPositionV3::new(taker.to_bytes(), taker_revision)
        .map_err(|error| Error::new(format!("the taker's delta position: {error:?}")))?;
    let (positions, dealer_index, taker_index, first_position, second_position) =
        if dealer_position_entry.owner() < taker_position_entry.owner() {
            (
                [dealer_position_entry, taker_position_entry],
                0_u32,
                1_u32,
                dealer_position,
                taker_position,
            )
        } else {
            (
                [taker_position_entry, dealer_position_entry],
                1_u32,
                0_u32,
                taker_position,
                dealer_position,
            )
        };
    let width = usize::from(fund.outcome_count);
    let mut aggregate_deltas = Vec::with_capacity(coordinates.claim_count as usize);
    for outcome in 0..coordinates.claim_count as usize {
        aggregate_deltas.push(delta_v1(if outcome < width {
            i128::from(request.mint)
        } else {
            0
        })?);
    }
    let mut rows = Vec::with_capacity(2 * width);
    for outcome in 0..width {
        let index = u32::try_from(outcome).map_err(|_| Error::new("outcome index overflow"))?;
        for (position_index, value) in [
            (dealer_index, request.dealer_delta(outcome)),
            (taker_index, request.taker_delta(outcome)),
        ] {
            if value != 0 {
                rows.push(
                    PositionDeltaV3::new(
                        PositionDeltaInputV3 {
                            position_index,
                            outcome: index,
                            delta: delta_v1(value)?,
                        },
                        2,
                        coordinates.claim_count,
                    )
                    .map_err(|error| Error::new(format!("one Position delta row: {error:?}")))?,
                );
            }
        }
    }
    if rows.is_empty() {
        return Err(Error::new(
            "this fill moves no claim, so the route invokes no Claims delta and the frame it \
             would still have to carry cannot be authenticated; ask for a fill that moves one",
        ));
    }
    let construction =
        ValidatedSignedDeltaConstructionV3::new(input, &positions, &aggregate_deltas, &rows)
            .map_err(|error| Error::new(format!("the signed-delta plan: {error:?}")))?;
    let packet_bytes = construction
        .encoded_bytes()
        .map_err(|error| Error::new(format!("signed-delta packet width: {error:?}")))?;
    let mut packet = vec![0_u8; packet_bytes];
    construction
        .encode_into(&mut packet)
        .map_err(|error| Error::new(format!("signed-delta packet: {error:?}")))?;
    let authority = coordinates.authority(fund_key.to_bytes(), hash(&packet).to_bytes())?;

    let spec = SignedDeltaFrameSpecV3::new(2)
        .map_err(|error| Error::new(format!("signed-delta frame: {error:?}")))?;
    let width = spec
        .account_count()
        .map_err(|error| Error::new(format!("signed-delta frame width: {error:?}")))?;
    let mut metas = Vec::with_capacity(usize::from(width));
    for index in 0..width {
        let account = spec
            .account(index)
            .map_err(|error| Error::new(format!("signed-delta coordinate {index}: {error:?}")))?;
        let key = match account.role() {
            ClaimsFrameRoleV1::CallerAuthority => authority,
            ClaimsFrameRoleV1::SignedDeltaPosition(0) => first_position,
            ClaimsFrameRoleV1::SignedDeltaPosition(1) => second_position,
            other => coordinates.common_claims_key(other)?,
        };
        metas.push(if account.privileges().writable() {
            AccountMeta::new(key, false)
        } else {
            AccountMeta::new_readonly(key, false)
        });
    }
    Ok(SignedDeltaWindowV1 { metas, authority })
}

fn delta_v1(value: i128) -> Result<SignedDeltaV3> {
    let magnitude = u64::try_from(value.unsigned_abs())
        .map_err(|_| Error::new("a claim delta exceeds a u64"))?;
    let direction = if value > 0 {
        DeltaDirectionV3::Credit
    } else if value < 0 {
        DeltaDirectionV3::Debit
    } else {
        DeltaDirectionV3::Neutral
    };
    SignedDeltaV3::new(direction, magnitude)
        .map_err(|error| Error::new(format!("one signed delta: {error:?}")))
}

// ---------------------------------------------------------------- withdraw

/// `DealerWithdraw`: fourteen accounts, then one Custody `Transfer` window.
fn plan_withdraw(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    coordinates: &CoordinatesV1,
) -> Result<PlanV1> {
    let sponsor = arguments
        .sponsor
        .ok_or_else(|| Error::new("--sponsor is required"))?;
    let sponsor_token = arguments
        .sponsor_token
        .ok_or_else(|| Error::new("--sponsor-token is required"))?;
    let amount = arguments
        .amount
        .ok_or_else(|| Error::new("--amount is required"))?;

    let fund_key = coordinates.fund(&arguments.dealer_id);
    let (fund, _) = read_fund(rpc, fund_key, coordinates, arguments.dealer_id)?;
    if fund.sponsor != sponsor.to_bytes() {
        return Err(Error::new(format!(
            "{sponsor} is not this fund's sponsor of record; the withdrawal would refuse as Sponsor"
        )));
    }
    let rule_key = coordinates.rule(&arguments.dealer_id);
    let rule = read_rule(rpc, rule_key, fund)?;
    let dealer_position = coordinates.position(fund_key)?;
    let inventory = read_inventory(
        rpc,
        coordinates,
        dealer_position,
        fund_key,
        fund.outcome_count,
    )?;
    let w = potential(rule.parameters, &inventory)
        .map_err(|error| Error::new(format!("the potential at this inventory: {error:?}")))?;
    // The route rounds the request UP to claim units; the floor is checked
    // against that rounded number and only while the Market is Open.
    let mut amount_claims = amount / fund.claim_unit_atoms;
    if amount_claims.saturating_mul(fund.claim_unit_atoms) < amount {
        amount_claims = amount_claims
            .checked_add(1)
            .ok_or_else(|| Error::new("claim-unit rounding overflow"))?;
    }
    if coordinates.phase == Phase::Open {
        dclutch_trading::scoring_rule::withdraw_admissible(fund.cash_claims(), w, amount_claims)
            .map_err(|error| {
                Error::new(format!(
                    "the withdrawal is above the floor Φ = cash + Ŵ: {error:?}"
                ))
            })?;
    }
    if amount > fund.cash {
        return Err(Error::new(format!(
            "the withdrawal asks {amount} atoms of a fund holding {}",
            fund.cash
        )));
    }

    let request = DealerWithdrawRequestV1 {
        market: coordinates.market.to_bytes(),
        dealer_id: arguments.dealer_id,
        expected_fund_revision: fund.revision,
        amount,
    }
    .to_bytes()
    .map_err(|error| Error::new(format!("withdrawal request: {error:?}")))?;
    let parent = hash(&request).to_bytes();

    let vault = coordinates.vault(fund_key);
    let replay = coordinates.replay(fund_key.to_bytes());
    let revision = replay_revision(rpc, replay, "the fund's Custody replay")?;
    let leg = custody_transfer_leg_v1(
        coordinates,
        fund_key.to_bytes(),
        replay,
        revision,
        vault,
        sponsor_token,
        CompartmentV1::TradingPrincipal,
        CompartmentV1::External,
        [0; 32],
        sponsor.to_bytes(),
        fund_key.to_bytes(),
        [0; 32],
        parent,
        0,
        amount,
    )?;

    let mut keys = vec![None; generated::WITHDRAW_ACCOUNT_COUNT];
    keys[generated::WITHDRAW_SPONSOR_ACCOUNT] = Some(sponsor);
    keys[generated::WITHDRAW_FUND_ACCOUNT] = Some(fund_key);
    keys[generated::WITHDRAW_RULE_ACCOUNT] = Some(rule_key);
    keys[generated::WITHDRAW_MARKET_ACCOUNT] = Some(coordinates.market);
    keys[generated::WITHDRAW_DEALER_POSITION_ACCOUNT] = Some(dealer_position);
    keys[generated::WITHDRAW_SPONSOR_TOKEN_ACCOUNT] = Some(sponsor_token);
    keys[generated::WITHDRAW_VAULT_ACCOUNT] = Some(vault);
    keys[generated::WITHDRAW_CUSTODY_REPLAY_ACCOUNT] = Some(replay);
    keys[generated::WITHDRAW_CUSTODY_AUTHORITY_ACCOUNT] = Some(coordinates.custody_authority);
    keys[generated::WITHDRAW_CUSTODY_PROGRAM_ACCOUNT] = Some(coordinates.custody_program);
    keys[generated::WITHDRAW_MINT_ACCOUNT] = Some(coordinates.mint);
    keys[generated::WITHDRAW_TOKEN_PROGRAM_ACCOUNT] = Some(coordinates.token_program);
    keys[generated::WITHDRAW_ACTIVATION_CACHE_ACCOUNT] = Some(coordinates.activation_cache);
    keys[generated::WITHDRAW_REGISTRY_PROGRAM_ACCOUNT] = Some(coordinates.registry);
    let prefix = prefix_v1(&keys, withdraw_privileges_v1)?;
    let windows = custody_transfer_window_v1(coordinates, &leg)?;

    let mut accounts = prefix;
    accounts.extend(windows);
    Ok(PlanV1 {
        route: RouteV1::Withdraw,
        instruction: Instruction {
            program_id: coordinates.trading_program,
            accounts,
            data: request.to_vec(),
        },
        request: request.to_vec(),
        routed: false,
        fund: fund_key,
        rule: rule_key,
        quote: coordinates.quote(&arguments.dealer_id),
        dealer_position,
        expected_fund_revision: fund.revision,
        authorities: vec![(
            "custody-withdraw".to_string(),
            leg.authority.unwrap_or(system_program::ID),
        )],
        facts: json!({
            "sponsor": sponsor.to_string(),
            "sponsorToken": sponsor_token.to_string(),
            "amountAtoms": amount,
            "amountClaimUnits": amount_claims,
            "cashAtoms": fund.cash,
            "cashClaimUnits": fund.cash_claims(),
            "potentialMinimum": w.minimum,
            "potentialCost": w.cost,
            "floorClaimUnits": i128::from(fund.cash_claims()) + w.value(),
            "vault": vault.to_string(),
        }),
        dealer_pays: 0,
        dealer_receives: 0,
    })
}

// ------------------------------------------------------------- the readback

/// What the chain says the route did, joined to the receipt shape it produced.
struct PostStateV1 {
    revision: u64,
    cash: u64,
    inventory_minimum: u64,
    liquidity_cost: u64,
    quote_fund_revision: Option<u64>,
    receipt_digest: [u8; 32],
}

/// Read the fund back and hold it to the receipt this route would have emitted.
///
/// The send landing is not the proof. `DealerReceiptV1::authenticate_for_request`
/// joins the request bytes and the fund bytes into one statement, and a fund
/// that did not move to the revision this plan named fails that join.
fn read_back(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    coordinates: &CoordinatesV1,
    plan: &PlanV1,
) -> Result<PostStateV1> {
    let account = rpc.required_account(plan.fund, "Dealer fund after the route")?;
    let fund = DealerFundV1::decode(&account.data)
        .map_err(|error| Error::new(format!("Dealer fund {}: {error:?}", plan.fund)))?;
    let advances = matches!(plan.route, RouteV1::Fill | RouteV1::Withdraw);
    let expected = if advances {
        plan.expected_fund_revision
            .checked_add(1)
            .ok_or_else(|| Error::new("fund revision overflow"))?
    } else {
        plan.expected_fund_revision
    };
    if fund.revision != expected {
        return Err(Error::new(format!(
            "the {} landed and left the fund at revision {}, not {expected}",
            plan.route.label(),
            fund.revision
        )));
    }
    if fund.dealer_id != arguments.dealer_id || fund.market != coordinates.market.to_bytes() {
        return Err(Error::new(
            "the fund read back belongs to another Market or Dealer",
        ));
    }
    let receipt = DealerReceiptV1::from_fund(
        plan.route.receipt_route(),
        &plan.request,
        fund,
        &account.data,
        plan.dealer_pays,
        plan.dealer_receives,
    )
    .authenticate_for_request(&plan.request, &account.data)
    .map_err(|error| {
        Error::new(format!(
            "the fund read back does not join this request's receipt: {error:?}"
        ))
    })?;
    println!("fund revision after  {}", fund.revision);
    println!("fund cash after      {}", fund.cash);

    // Founding and quote both rewrite the quote account; reading it back is
    // what says the price series became a chain fact rather than a log line.
    let quote_fund_revision = match plan.route {
        RouteV1::Found | RouteV1::Quote => {
            let quote = rpc.required_account(plan.quote, "Dealer quote after the route")?;
            let decoded = DealerQuoteV1::decode(&quote.data)
                .map_err(|error| Error::new(format!("Dealer quote {}: {error:?}", plan.quote)))?;
            if !decoded.is_fresh(fund) {
                return Err(Error::new(format!(
                    "the quote at {} was written at fund revision {}, not the fund's {}",
                    plan.quote, decoded.fund_revision, fund.revision
                )));
            }
            println!("quote fund revision  {} (fresh)", decoded.fund_revision);
            Some(decoded.fund_revision)
        }
        RouteV1::Fill | RouteV1::Withdraw => None,
    };

    Ok(PostStateV1 {
        revision: fund.revision,
        cash: fund.cash,
        inventory_minimum: fund.inventory_minimum,
        liquidity_cost: fund.liquidity_cost,
        quote_fund_revision,
        receipt_digest: receipt.fund_digest,
    })
}

fn report(coordinates: &CoordinatesV1, plan: &PlanV1) {
    println!("== {} ==", plan.route.label());
    println!("market               {}", coordinates.market);
    println!("generation           {}", coordinates.generation);
    println!(
        "release set          {}",
        hex_lower(&coordinates.release_set)
    );
    println!("trading program      {}", coordinates.trading_program);
    println!("claims aggregate     {}", coordinates.aggregate);
    println!(
        "custody context      {}",
        hex_lower(&coordinates.custody_context)
    );
    println!("hoard                {}", coordinates.hoard);
    println!("collateral mint      {}", coordinates.mint);
    println!("ordinary outcomes    {}", coordinates.outcome_count);
    println!("claim unit atoms     {}", coordinates.claim_unit_atoms);
    println!("fund                 {}", plan.fund);
    println!("rule                 {}", plan.rule);
    println!("quote                {}", plan.quote);
    println!("dealer position      {}", plan.dealer_position);
    println!("fund revision read   {}", plan.expected_fund_revision);
    for (label, authority) in &plan.authorities {
        println!("authority {label:<10} {authority}");
    }
    println!("frame accounts       {}", plan.instruction.accounts.len());
    println!("instruction bytes    {}", plan.instruction.data.len());
    println!(
        "packet routing       {}",
        if plan.routed {
            "one frozen address lookup table this run publishes"
        } else {
            "none; the frame fits a legacy packet inline"
        }
    );
    if let Some(object) = plan.facts.as_object() {
        for (key, value) in object {
            println!("{key:<20} {value}");
        }
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Write what this submission was, whether or not it was sent.
fn write_evidence(
    path: &Path,
    coordinates: &CoordinatesV1,
    plan: &PlanV1,
    signer: Option<Pubkey>,
    landed: Option<(&TransactionEvidence, &PostStateV1)>,
    cluster: &str,
    expected_cluster: ExpectedClusterV1,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-scoring-dealer-evidence-v1",
        "cluster": cluster,
        "route": plan.route.command(expected_cluster),
        "market": coordinates.market.to_string(),
        "generation": coordinates.generation,
        "releaseSet": hex_lower(&coordinates.release_set),
        "aggregate": coordinates.aggregate.to_string(),
        "custodyContext": hex_lower(&coordinates.custody_context),
        "hoard": coordinates.hoard.to_string(),
        "mint": coordinates.mint.to_string(),
        "outcomeCount": coordinates.outcome_count,
        "claimUnitAtoms": coordinates.claim_unit_atoms,
        "fund": plan.fund.to_string(),
        "rule": plan.rule.to_string(),
        "quote": plan.quote.to_string(),
        "dealerPosition": plan.dealer_position.to_string(),
        "expectedFundRevision": plan.expected_fund_revision,
        "requestDigest": hex_lower(&hash(&plan.request).to_bytes()),
        "frameAccounts": plan.instruction.accounts.len(),
        "callerAuthorities": plan
            .authorities
            .iter()
            .map(|(label, key)| json!({ "window": label, "authority": key.to_string() }))
            .collect::<Vec<_>>(),
        "facts": plan.facts,
        // Fixed width across preflight and execution: a null signer means no
        // key was opened and no transaction exists.
        "signer": signer.map(|key| key.to_string()),
        "landed": landed.map(|(evidence, poststate)| json!({
            "signature": evidence.signature,
            "slot": evidence.slot,
            "computeUnitsConsumed": evidence.compute_units_consumed,
            "feeLamports": evidence.fee_lamports,
            "fundRevisionAfter": poststate.revision,
            "fundCashAfter": poststate.cash,
            "fundInventoryMinimumAfter": poststate.inventory_minimum,
            "fundLiquidityCostAfter": poststate.liquidity_cost,
            "quoteFundRevision": poststate.quote_fund_revision,
            "fundDigestAfter": hex_lower(&poststate.receipt_digest),
        })),
    });
    authenticate_signer_evidence_v1(&document, signer, landed.map(|(evidence, _)| evidence))?;
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )?;
    println!("evidence             {}", path.display());
    Ok(())
}

/// Bind the fixed evidence signer to the exact transaction tuple the RPC owner
/// returned. The signer arrives as a `Pubkey` already derived from the opened
/// key, never as a filename or a caller-authored string.
fn authenticate_signer_evidence_v1(
    document: &Value,
    expected_signer: Option<Pubkey>,
    expected_landed: Option<&TransactionEvidence>,
) -> Result<()> {
    let recorded_signer = document
        .get("signer")
        .ok_or_else(|| Error::new("scoring Dealer evidence omitted its fixed signer field"))?;
    let recorded_landed = document
        .get("landed")
        .ok_or_else(|| Error::new("scoring Dealer evidence omitted its fixed landed field"))?;
    match (expected_signer, expected_landed) {
        (None, None) => {
            if !recorded_signer.is_null() || !recorded_landed.is_null() {
                return Err(Error::new(
                    "scoring Dealer preflight evidence named a signer or a transaction",
                ));
            }
        }
        (Some(signer), Some(transaction)) => {
            if transaction.error.is_some()
                || recorded_signer.as_str() != Some(signer.to_string().as_str())
                || recorded_landed.get("signature").and_then(Value::as_str)
                    != Some(transaction.signature.as_str())
                || recorded_landed.get("slot").and_then(Value::as_u64) != Some(transaction.slot)
                || recorded_landed
                    .get("computeUnitsConsumed")
                    .and_then(Value::as_u64)
                    != transaction.compute_units_consumed
                || recorded_landed.get("feeLamports").and_then(Value::as_u64)
                    != transaction.fee_lamports
            {
                return Err(Error::new(
                    "scoring Dealer evidence signer or landed transaction tuple changed",
                ));
            }
        }
        (Some(_), None) => {
            return Err(Error::new(
                "scoring Dealer execute evidence omitted its landed transaction",
            ));
        }
        (None, Some(_)) => {
            return Err(Error::new(
                "scoring Dealer landed evidence omitted its exact transaction signer",
            ));
        }
    }
    Ok(())
}

fn parse(
    route: RouteV1,
    expected_cluster: ExpectedClusterV1,
    arguments: Vec<String>,
) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut market = None;
    let mut campaign_report = None;
    let mut dealer_id = None;
    let mut evidence = None;
    let mut execute = false;
    let mut sponsor = None;
    let mut sponsor_token = None;
    let mut sponsor_keypair = None;
    let mut fee_payer = None;
    let mut fee_payer_keypair = None;
    let mut taker = None;
    let mut taker_token = None;
    let mut taker_keypair = None;
    let mut buy_outcome = None;
    let mut buy_claims = None;
    let mut liquidity = None;
    let mut scale = None;
    let mut tolerance = None;
    let mut deposit = None;
    let mut claim_unit_atoms = None;
    let mut amount = None;
    let mut cursor = arguments.into_iter();
    while let Some(flag) = cursor.next() {
        let mut value = || {
            cursor
                .next()
                .ok_or_else(|| Error::new(format!("{flag} requires a value")))
        };
        match flag.as_str() {
            // The cohort generator injects the origin pair immediately after
            // the verb, so every arm accepts both and no row ever writes them.
            "--rpc-url" => rpc_url = Some(value()?),
            "--i-mean-devnet" => acknowledgment = Some(value()?),
            "--market" => market = Some(parse_pubkey(&value()?, "--market")?),
            "--campaign-report" => campaign_report = Some(PathBuf::from(value()?)),
            "--dealer-id" => dealer_id = Some(hex32(&value()?)?),
            "--evidence" => evidence = Some(PathBuf::from(value()?)),
            "--sponsor" => sponsor = Some(parse_pubkey(&value()?, "--sponsor")?),
            "--sponsor-token" => sponsor_token = Some(parse_pubkey(&value()?, "--sponsor-token")?),
            "--sponsor-keypair" => sponsor_keypair = Some(PathBuf::from(value()?)),
            "--fee-payer" => fee_payer = Some(parse_pubkey(&value()?, "--fee-payer")?),
            "--fee-payer-keypair" => fee_payer_keypair = Some(PathBuf::from(value()?)),
            "--taker" => taker = Some(parse_pubkey(&value()?, "--taker")?),
            "--taker-token" => taker_token = Some(parse_pubkey(&value()?, "--taker-token")?),
            "--taker-keypair" => taker_keypair = Some(PathBuf::from(value()?)),
            "--buy-outcome" => buy_outcome = Some(parse_u8(&value()?, "--buy-outcome")?),
            "--buy-claims" => buy_claims = Some(parse_u64(&value()?, "--buy-claims")?),
            "--liquidity" => liquidity = Some(parse_u64(&value()?, "--liquidity")?),
            "--scale" => scale = Some(parse_u64(&value()?, "--scale")?),
            "--tolerance" => tolerance = Some(parse_u64(&value()?, "--tolerance")?),
            "--deposit" => deposit = Some(parse_u64(&value()?, "--deposit")?),
            "--claim-unit-atoms" => {
                claim_unit_atoms = Some(parse_u64(&value()?, "--claim-unit-atoms")?);
            }
            "--amount" => amount = Some(parse_u64(&value()?, "--amount")?),
            "--execute" => execute = true,
            other => return Err(Error::new(format!("unknown flag: {other}"))),
        }
    }
    let arguments = ArgumentsV1 {
        route,
        expected_cluster,
        rpc_url: rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?,
        acknowledgment,
        market: market.ok_or_else(|| Error::new("--market is required"))?,
        campaign_report: campaign_report
            .ok_or_else(|| Error::new("--campaign-report is required"))?,
        dealer_id: dealer_id.ok_or_else(|| Error::new("--dealer-id is required"))?,
        evidence: evidence.ok_or_else(|| Error::new("--evidence is required"))?,
        execute,
        sponsor,
        sponsor_token,
        sponsor_keypair,
        fee_payer,
        fee_payer_keypair,
        taker,
        taker_token,
        taker_keypair,
        buy_outcome,
        buy_claims,
        liquidity,
        scale,
        tolerance,
        deposit,
        claim_unit_atoms,
        amount,
    };
    require_route_flags_v1(&arguments)?;
    Ok(arguments)
}

/// Refuse another verb's flags before a socket, a file, or a key is opened.
///
/// A stray `--amount` on a founding is not harmless: it is a caller who
/// believes this run withdraws something, and the run that silently ignored it
/// would report a founding they did not ask for.
fn require_route_flags_v1(arguments: &ArgumentsV1) -> Result<()> {
    let foreign: &[(&str, bool)] = match arguments.route {
        RouteV1::Found => &[
            ("--amount", arguments.amount.is_some()),
            ("--taker", arguments.taker.is_some()),
            ("--buy-outcome", arguments.buy_outcome.is_some()),
            ("--buy-claims", arguments.buy_claims.is_some()),
            ("--fee-payer", arguments.fee_payer.is_some()),
        ],
        RouteV1::Quote => &[
            ("--amount", arguments.amount.is_some()),
            ("--deposit", arguments.deposit.is_some()),
            ("--liquidity", arguments.liquidity.is_some()),
            ("--taker", arguments.taker.is_some()),
            ("--sponsor", arguments.sponsor.is_some()),
        ],
        RouteV1::Fill => &[
            ("--amount", arguments.amount.is_some()),
            ("--deposit", arguments.deposit.is_some()),
            ("--liquidity", arguments.liquidity.is_some()),
            ("--sponsor", arguments.sponsor.is_some()),
            ("--fee-payer", arguments.fee_payer.is_some()),
        ],
        RouteV1::Withdraw => &[
            ("--deposit", arguments.deposit.is_some()),
            ("--liquidity", arguments.liquidity.is_some()),
            ("--taker", arguments.taker.is_some()),
            ("--buy-claims", arguments.buy_claims.is_some()),
            ("--fee-payer", arguments.fee_payer.is_some()),
        ],
    };
    if let Some((flag, _)) = foreign.iter().find(|(_, present)| *present) {
        return Err(Error::new(format!(
            "{flag} belongs to another scoring Dealer verb, not to {}",
            arguments.route.command(arguments.expected_cluster)
        )));
    }
    Ok(())
}

fn parse_pubkey(value: &str, flag: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{flag}: {error}")))
}

fn parse_u64(value: &str, flag: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .map_err(|error| Error::new(format!("{flag}: {error}")))
}

fn parse_u8(value: &str, flag: &str) -> Result<u8> {
    value
        .parse::<u8>()
        .map_err(|error| Error::new(format!("{flag}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Vec<String> {
        let mut value = vec![
            "--rpc-url".into(),
            "https://api.devnet.solana.com".into(),
            "--market".into(),
            "11111111111111111111111111111111".into(),
            "--campaign-report".into(),
            "/nonexistent/campaign-open.json".into(),
            "--dealer-id".into(),
            "01".repeat(32),
            "--evidence".into(),
            "/nonexistent/evidence.json".into(),
        ];
        value.extend(extra.iter().map(|entry| (*entry).to_string()));
        value
    }

    /// Every arm takes the origin pair the cohort generator injects right after
    /// the verb, and no arm treats either as unknown.
    #[test]
    fn every_arm_accepts_the_injected_origin_pair() {
        for route in [
            RouteV1::Found,
            RouteV1::Quote,
            RouteV1::Fill,
            RouteV1::Withdraw,
        ] {
            let parsed = parse(
                route,
                ExpectedClusterV1::Devnet,
                args(&["--i-mean-devnet", "SomeGenesisHash"]),
            )
            .expect("the injected origin pair is admitted");
            assert_eq!(parsed.acknowledgment.as_deref(), Some("SomeGenesisHash"));
            assert_eq!(parsed.rpc_url, "https://api.devnet.solana.com");
        }
    }

    /// A public arm without the acknowledgment refuses by that exact name,
    /// before a socket is opened.
    #[test]
    fn a_public_arm_refuses_without_the_acknowledgment() {
        let refusal = run(
            RouteV1::Quote,
            ExpectedClusterV1::Devnet,
            args(&["--fee-payer", "11111111111111111111111111111111"]),
        )
        .expect_err("the devnet arm must refuse without an acknowledgment");
        assert_eq!(
            format!("{refusal}"),
            "--i-mean-devnet GENESIS_HASH is required to reach a public cluster through \
             devnet-dealer-quote-v1"
        );
    }

    #[test]
    fn every_local_arm_admits_loopback_before_reading_any_files() {
        for route in [
            RouteV1::Found,
            RouteV1::Quote,
            RouteV1::Fill,
            RouteV1::Withdraw,
        ] {
            let mut argv = args(&[]);
            argv[1] = "http://127.0.0.1:20890".into();
            let parsed = parse(route, ExpectedClusterV1::OwnedLoopback, argv).unwrap();
            assert_eq!(
                authenticate_origin(&parsed).unwrap(),
                ClusterOriginV1::Loopback {
                    url: "http://127.0.0.1:20890/".into(),
                    port: 20890
                }
            );
            assert!(
                route
                    .command(parsed.expected_cluster)
                    .starts_with("local-private-validator-")
            );
        }
    }

    #[test]
    fn local_arm_refuses_even_acknowledged_devnet_before_any_io() {
        let refusal = run(
            RouteV1::Quote,
            ExpectedClusterV1::OwnedLoopback,
            args(&["--i-mean-devnet", crate::cluster::DEVNET_GENESIS_HASH]),
        )
        .expect_err("the local command cannot leave its validator");
        assert_eq!(
            format!("{refusal}"),
            "private-validator executor requires an owned loopback validator and refuses every external origin"
        );
    }

    #[test]
    fn local_arm_refuses_an_acknowledgment_on_loopback_before_any_io() {
        let mut argv = args(&["--i-mean-devnet", crate::cluster::DEVNET_GENESIS_HASH]);
        argv[1] = "http://127.0.0.1:20890".into();
        let parsed = parse(RouteV1::Quote, ExpectedClusterV1::OwnedLoopback, argv).unwrap();
        let refusal = authenticate_origin(&parsed).expect_err("conflicting origin intent");
        assert_eq!(
            format!("{refusal}"),
            "--i-mean-devnet was given for the loopback origin http://127.0.0.1:20890/. A loopback origin needs no acknowledgment, so one of the two is a mistake and this refuses rather than guessing which."
        );
    }

    /// Another verb's flag is a refusal, not a silently ignored word.
    #[test]
    fn a_foreign_flag_is_refused_by_name() {
        let refusal = parse(
            RouteV1::Found,
            ExpectedClusterV1::Devnet,
            args(&["--amount", "7"]),
        )
        .expect_err("a withdrawal flag has no place on a founding");
        assert_eq!(
            format!("{refusal}"),
            "--amount belongs to another scoring Dealer verb, not to devnet-dealer-found-v1"
        );
    }

    /// The four frame widths this module fills are the Lean's, restated
    /// nowhere: the prefix builder walks the emitted privilege table to its end
    /// and refuses a plan that named fewer coordinates.
    #[test]
    fn the_prefix_builder_is_the_width_the_lean_declares() {
        for (count, privileges) in [
            (
                generated::FOUND_ACCOUNT_COUNT,
                found_privileges_v1 as fn(usize) -> Option<(bool, bool)>,
            ),
            (generated::QUOTE_ACCOUNT_COUNT, quote_privileges_v1),
            (generated::FILL_ACCOUNT_COUNT, fill_privileges_v1),
            (generated::WITHDRAW_ACCOUNT_COUNT, withdraw_privileges_v1),
        ] {
            let keys = vec![Some(Pubkey::new_unique()); count];
            let metas = prefix_v1(&keys, privileges).expect("the exact width is admitted");
            assert_eq!(metas.len(), count);
            let short = prefix_v1(&keys[..count - 1], privileges)
                .expect_err("a frame one coordinate short is refused");
            assert_eq!(
                format!("{short}"),
                "the frame declares more coordinates than this plan named"
            );
            // A coordinate a plan never wrote is `None`. The System Program
            // itself is the default pubkey, so an address cannot serve as the
            // completeness sentinel.
            let mut unnamed = keys.clone();
            unnamed[count - 1] = None;
            let refusal =
                prefix_v1(&unnamed, privileges).expect_err("an unnamed coordinate is refused");
            assert_eq!(
                format!("{refusal}"),
                format!("this plan left frame coordinate {} unnamed", count - 1)
            );
        }
    }

    #[test]
    fn the_prefix_builder_accepts_a_named_system_program() {
        let mut keys = vec![Some(Pubkey::new_unique()); generated::FOUND_ACCOUNT_COUNT];
        keys[generated::FOUND_SYSTEM_PROGRAM_ACCOUNT] = Some(system_program::ID);
        let metas = prefix_v1(&keys, found_privileges_v1)
            .expect("System Program is a named account, despite its zero pubkey");
        assert_eq!(
            metas[generated::FOUND_SYSTEM_PROGRAM_ACCOUNT].pubkey,
            system_program::ID
        );
    }

    /// Coordinate zero of every frame is its one signer, and it is the only
    /// one: the privilege tables say so, and this module reads them rather
    /// than asserting it.
    #[test]
    fn each_frame_has_exactly_one_signer_at_coordinate_zero() {
        for (count, privileges) in [
            (
                generated::FOUND_ACCOUNT_COUNT,
                found_privileges_v1 as fn(usize) -> Option<(bool, bool)>,
            ),
            (generated::QUOTE_ACCOUNT_COUNT, quote_privileges_v1),
            (generated::FILL_ACCOUNT_COUNT, fill_privileges_v1),
            (generated::WITHDRAW_ACCOUNT_COUNT, withdraw_privileges_v1),
        ] {
            let signers = (0..count)
                .filter(|index| privileges(*index).is_some_and(|(_, signer)| signer))
                .collect::<Vec<_>>();
            assert_eq!(signers, vec![0]);
        }
    }

    /// The Custody `Transfer` window is built from Custody's own frame spec,
    /// so a reordering there moves this window rather than breaking it.
    #[test]
    fn the_custody_transfer_window_is_the_frame_spec() {
        let spec = CustodyFrameSpecV1::new(OperationV1::Transfer);
        assert_eq!(spec.account_count(), TRANSFER_ACCOUNT_COUNT_V1);
        let metas = custody_window_v1(spec, |_| Ok(Pubkey::new_unique()))
            .expect("every Transfer coordinate has a key");
        assert_eq!(metas.len(), usize::from(TRANSFER_ACCOUNT_COUNT_V1));
        // No window coordinate may sign at the top level: the route's own
        // `parse_prefix` refuses one, and every child signature is the
        // program's PDA under `invoke_signed`.
        assert!(metas.iter().all(|meta| !meta.is_signer));
        for index in 0..TRANSFER_ACCOUNT_COUNT_V1 {
            let expected = spec.account(index).expect("coordinate").privileges();
            assert_eq!(metas[usize::from(index)].is_writable, expected.writable());
        }
    }

    /// `cashLegs`' two invariants: the Hoard receives exactly the mint's par,
    /// and the fund moves by exactly the Dealer's debit.
    #[test]
    fn the_cash_legs_conserve_the_par_and_the_debit() {
        for (mint, pays) in [(0_u64, 0_u64), (7, 3), (3, 7), (300, 100), (1_000, 1_000)] {
            let legs = cash_legs_v1(mint, pays, 0);
            assert_eq!(legs.taker_to_hoard + legs.fund_to_hoard, mint);
            assert_eq!(legs.fund_to_hoard + legs.fund_to_taker, pays);
            assert_eq!(legs.taker_to_fund, 0);
        }
        let receiving = cash_legs_v1(300, 0, 50);
        assert_eq!(receiving.taker_to_hoard, 300);
        assert_eq!(receiving.fund_to_hoard, 0);
        assert_eq!(receiving.fund_to_taker, 0);
        assert_eq!(receiving.taker_to_fund, 50);
    }

    /// The fill's Claims window is the fixed signed-delta frame plus the two
    /// Positions, and the constant this module carries is that number.
    #[test]
    fn the_fill_claims_window_is_the_fixed_frame_plus_two_positions() {
        let spec = SignedDeltaFrameSpecV3::new(2).expect("two positions");
        assert_eq!(
            usize::from(spec.account_count().expect("width")),
            FILL_CLAIMS_WINDOW_ACCOUNTS
        );
    }

    /// A vacant PDA has no bytes, but Claims allocates these two exact layouts
    /// after DealerFound has paid their rent. Reusing the vacant lengths would
    /// make the request fail `ProtocolPositionRequestV2::validate` before the
    /// founding instruction can perform that prefund.
    #[test]
    fn vacant_dealer_admission_uses_post_allocation_rent_principals() {
        let rent = solana_sdk::rent::Rent::default();
        let claim_count = 2;
        let vacant = plan_admit_rents_v1(&rent, claim_count, 0, 0).expect("canonical widths");
        let position_bytes =
            liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, claim_count)
                .expect("position width");
        assert_eq!(
            vacant.position_principal,
            rent.minimum_balance(position_bytes)
        );
        assert_eq!(
            vacant.admission_principal,
            rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
        );
        assert_eq!(vacant.position_lamports, vacant.position_principal);
        assert_eq!(vacant.admission_lamports, vacant.admission_principal);
        assert_ne!(vacant.position_principal, rent.minimum_balance(0));
        assert_ne!(vacant.admission_principal, rent.minimum_balance(0));

        let donated = plan_admit_rents_v1(
            &rent,
            claim_count,
            vacant.position_principal + 1,
            vacant.admission_principal + 1,
        )
        .expect("donated canonical widths");
        assert_eq!(donated.position_lamports, vacant.position_principal + 1);
        assert_eq!(donated.admission_lamports, vacant.admission_principal + 1);
    }

    #[test]
    fn preflight_evidence_cannot_claim_a_signer() {
        let claimed = json!({
            "signer": Pubkey::new_unique().to_string(),
            "landed": Value::Null,
        });
        let refusal = authenticate_signer_evidence_v1(&claimed, None, None)
            .expect_err("a preflight that named a signer is refused");
        assert_eq!(
            format!("{refusal}"),
            "scoring Dealer preflight evidence named a signer or a transaction"
        );
    }

    #[test]
    fn execute_evidence_must_carry_its_landed_transaction() {
        let document = json!({ "signer": Pubkey::new_unique().to_string(), "landed": Value::Null });
        let refusal = authenticate_signer_evidence_v1(&document, Some(Pubkey::new_unique()), None)
            .expect_err("an execute run with no transaction is refused");
        assert_eq!(
            format!("{refusal}"),
            "scoring Dealer execute evidence omitted its landed transaction"
        );
    }
}
