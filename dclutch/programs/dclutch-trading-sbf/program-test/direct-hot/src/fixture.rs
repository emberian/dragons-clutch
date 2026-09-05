//! Canonical executable Direct Hot account fixture.
//!
//! This module is the sole chain-account constructor for the selected Direct
//! ProgramTest campaign. It derives persisted records and PDAs from public
//! semantic-owner encoders, then packs the logical frame through the selected
//! [`AccountProfileV2`]. The parent Registry harness supplies only release-waist
//! accounts which it already owns.

use dclutch_claims::{
    CallerRole as ClaimsCallerRole,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
        LiabilityBasisPositionInputV2, encode_liability_basis_market_into_v2,
        encode_liability_basis_position_into_v2, put_liability_basis_market_bump_v2,
        put_liability_basis_position_bump_v2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
    sparse_native_transfer_v1::{SparseNativeTransferInputV1, SparseNativeTransferV1},
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_custody::token_svm::{LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES};
use dclutch_custody::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1,
    ContextV1, CustodyAuthoritySeedsV1, CustodyFrameRoleV1, CustodyFrameSpecV1,
    CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestLayoutV1, CustodyRequestV1,
    CustodyVaultSeedsV1, DELEGATED_CUSTODY_REQUEST_BYTES_V2, DelegatedCustodyRequestLayoutV2,
    DelegatedCustodyRequestV2, OperationV1,
};
use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_BUMP_HINT_COUNT_V1, HOT_BUMP_HINTS_OFFSET_V1,
        HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CONFIG_STAGING_ACCOUNT_V3,
        HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
        HOT_EFFECT_RAW_ACCOUNT_V3, HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_EXECUTION_ENVELOPE_BYTES_V3,
        HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        HOT_LIFECYCLE_RAW_ACCOUNT_V3, HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
        HOT_MANIFEST_RAW_ACCOUNT_V3, HOT_MANIFEST_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PORTFOLIO_STAGING_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PRODUCT_STAGING_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_STAGING_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
        HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_STRATEGY_STAGING_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_TRANSITION_STAGING_ACCOUNT_V3, HotBumpHintsV1, HotExecutionEnvelopeV3,
        SEALED_EXECUTION_FIXED_ALIASES_V3,
    },
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
    },
};
use dclutch_market::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity,
    PRODUCT_GRAPH_BUMP_COUNT, Phase, ProductGraphBumpsV1, Readiness, StateBumpsV1,
};
use dclutch_operator::hot_bump_miner::{
    HotBumpCorpusV1, hot_bump_hint_slot_name_v1, mine_hot_bump_hints_v1,
};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_product::payoff::runtime_v3::{
    BasisInputV3, BasisKindV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, basis_record_bytes_v3,
    compile_basis_v3, semantic_basis_preimage_v3,
};
use dclutch_product::{
    ContentId as ProductContentId, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{
    CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1, ExecutionRoleV1,
};
use dclutch_trading::{
    execution_v3::{
        DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3, DIRECT_REGISTRATION_REQUEST_BYTES_V3,
        DirectExecutionActionV3, DirectRegistrationRequestV3, DirectSignedParticipantV3,
        encode_header_v3,
    },
    intent_v2::CompactIntentV2,
    ordinary_effect_artifacts_v3::{
        DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3, DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
    },
    program_set_v4::{
        encode_direct_program_set_v2_atomic, encoded_direct_program_set_bytes_v4,
        validate_direct_register_buy_capability_v4, validate_direct_register_sell_capability_v4,
    },
    registered_account_artifacts_v4::{
        DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4,
        DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4, DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4,
        DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4, DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4,
        DIRECT_REGISTER_SELL_COLLATERAL_ACCOUNT_V4, DIRECT_REGISTER_SELL_FIXED_ACCOUNTS_V4,
        DirectRegisterBuyAccountProfileInputV4,
    },
    registered_bundle_v4::{
        DirectRegisterBuyHotBundleInputV4, DirectRegisterBuyHotBundleV4,
        DirectRegisterSellHotBundleInputV4, DirectRegisterSellHotBundleV4,
        build_direct_register_buy_hot_bundle_v4, build_direct_register_sell_hot_bundle_v4,
    },
    registered_requests_v4::encode_direct_registration_request_v3_atomic,
    registered_state_artifacts_v4::DirectRegisteredCreationChildRentWidthsV4,
    successor::{
        AuthenticatedCompactIntentV2, DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        DIRECT_FEE_DENOMINATOR_V1, DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_REGISTERED_RECORD_BYTES_V2,
        DirectCoordinatesV1, DirectExecutionConfigV1, DirectRootStateV1, MakerReplayFirstUseV1,
        MakerReplayObservationV1, MakerReplaySeedsV1, MakerReplayVacancyV1,
        RegisteredIntentSeedsV2, RegisteredRecordFirstUseV2, register_intent_v2,
    },
};
use dclutch_vm::account_profile::v2::AccountProfileV2;
use dclutch_vm::capability_seal::{
    CAPABILITY_SEAL_BYTES_V1, CAPABILITY_SEAL_ROW_COUNT_V1, CapabilitySealKeyV1,
    SealedDescriptorClosureV1, SealedRecordRowV1, SealedRoleV1,
};
use solana_account::Account;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_sdk_ids::bpf_loader_upgradeable;
use solana_sdk_ids::{system_program, sysvar};
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

use dclutch_trading::ordinary_geometry_v3::DirectOrdinaryGeometryV3;

use crate::{
    DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5, DirectHotArtifactFixtureV5,
    DirectHotDeploymentWidthsV5, DirectHotFixtureErrorV5, build_direct_hot_artifact_fixture_v5,
    chain::DirectHotInstallAccountV5,
};

const GENERATION: u64 = 9;
const PRICE_SCALE: u64 = 100;
/// The immutable development-market fee profile: 50 basis points per side.
const FEE_BPS: u16 = 50;
const FILL: u64 = 10;
const EXECUTION_PRICE: u64 = 50;
const CLAIMS_MARKET_REVISION: u64 = 8;
const SELLER_POSITION_REVISION: u64 = 9;
const BUYER_POSITION_REVISION: u64 = 10;
const CUSTODY_REVISION: u64 = 7;

/// The trade this fixture executes, and the ONE input that decides how many
/// Custody routes run.
///
/// # Why this is a fixture input and not a constant any more
///
/// It was a constant, and the constant was `FILL = 10` at `EXECUTION_PRICE =
/// 50` against a `PRICE_SCALE` of 100 -- gross 5. At the market's 50 basis
/// points per side that is `5 * 50 / 10_000`, which FLOORS TO ZERO. A zero
/// combined fee sets `seller_terminal` and clears both fee registers, so the
/// transition projects one live Custody route out of the four it declares, the
/// transaction makes ONE Custody CPI, and the fee leg -- the second Custody
/// route, its own caller authority, its own replay revision step, its own
/// delegated transfer -- has never executed in any measurement this repository
/// has ever taken. That was found on 2026-08-30
/// (`docs/evidence/DIRECT_HOT_CU_VARIANCE_CENSUS_2026-08-30.md`, finding 3) and
/// it is not a small caveat: a market that charges a fee runs a route whose
/// compute nobody had measured.
///
/// So the trade size is stated rather than assumed, and the two values below
/// are the two shapes the route actually has. `ZERO_FEE` reproduces the
/// historical fixture byte for byte -- every account, every address, every
/// signed preimage -- so nothing that was measured on it is invalidated by this
/// type existing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectTradeScenarioV1 {
    /// The Claims aggregate's supply of every outcome, which the seller holds
    /// in full -- so it is also the largest fill this market can trade.
    claim_supply: u64,
    /// Quantity filled, in claims; also each intent's `maximum_fill`.
    fill: u64,
    /// Execution price, scaled by `PRICE_SCALE`; also each intent's limit.
    ///
    /// The protocol requires `execution_price <= price_scale`, so this is a
    /// fraction of scale and `gross` can never exceed `fill`.
    execution_price: u64,
    /// The buyer's collateral balance, which must cover `gross + fee`.
    source_balance: u64,
}

impl DirectTradeScenarioV1 {
    /// The historical trade: gross 5, fee floors to zero, ONE Custody route.
    ///
    /// Every CU figure in `docs/evidence/DIRECT_HOT_CU_VARIANCE_CENSUS_2026-08-30.md`
    /// and every constant in `direct_hot_top_level_margin_gate.rs` belongs to
    /// this scenario.
    pub const ZERO_FEE: Self = Self {
        claim_supply: 100,
        fill: FILL,
        execution_price: EXECUTION_PRICE,
        source_balance: 100,
    };

    /// The smallest trade at this market's 50 bps whose fee does not floor away.
    ///
    /// # Why the SIZE moves and the price does not
    ///
    /// The obvious way to buy a nonzero fee is a higher price, and the protocol
    /// forbids it. The InlineOrdinary program requires
    /// `execution_price <= price_scale` -- the price is a FRACTION of scale,
    /// which is what a claim's price is -- so `gross = fill * price / scale` can
    /// never exceed the fill, and its exact division additionally requires
    /// the product to divide the scale exactly. At 50 basis points a nonzero fee
    /// needs `gross >= 200`, so it needs `fill >= 200`, and there is no price
    /// anywhere in the admissible range that gets there on a fill of 10.
    ///
    /// That is worth stating plainly because it is the real shape of the wall:
    /// the zero-fee measurement was not an unlucky constant, it was a trade two
    /// orders of magnitude too small for this market's own fee rate to bite.
    ///
    /// So this scenario keeps the price at 50 of 100 -- the same half the
    /// zero-fee scenario trades at -- and raises the size to 400 claims:
    /// `gross = 400 * 50 / 100 = 200`, each side's fee is `200 * 50 / 10_000 =
    /// 1`, seller net 199, combined fee 2, buyer debit 201. That is the smallest
    /// admissible fee-bearing trade at this rate, and smallest is right: the
    /// enable registers are booleans, so a larger trade buys nothing but a
    /// bigger number to explain. What matters is that `seller_net != 0` too,
    /// which selects `SellerIntermediate` + `FeeContinuation` -- the TWO-route
    /// shape a real fee-charging market runs -- rather than the single `FeeSole`
    /// route a fee with no seller leg would take.
    pub const FEE_BEARING: Self = Self {
        claim_supply: 1_000,
        fill: 400,
        execution_price: EXECUTION_PRICE,
        source_balance: 1_000,
    };

    /// The claims quantity this trade moves.
    #[must_use]
    pub const fn fill(self) -> u64 {
        self.fill
    }

    /// The scaled execution price, which is also both intents' limit.
    #[must_use]
    pub const fn execution_price(self) -> u64 {
        self.execution_price
    }

    /// The Claims supply this market carries, and the seller's whole position.
    #[must_use]
    pub const fn claim_supply(self) -> u64 {
        self.claim_supply
    }
}

/// Externally installed release-waist and deployment identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectHotChainInputV5 {
    /// Registry program owning finalized records and activation.
    pub registry_program: Pubkey,
    /// Current Trading program.
    pub trading_program: Pubkey,
    /// Current Core program.
    pub core_program: Pubkey,
    /// Current Claims program.
    pub claims_program: Pubkey,
    /// Current Custody program.
    pub custody_program: Pubkey,
    /// Rent program owning the Market-lifecycle RentCredit.
    pub rent_program: Pubkey,
    /// Exact immutable execution-release-set content identity.
    pub release_set: [u8; 32],
    /// Current complete activation cache.
    pub activation_cache: Pubkey,
    /// Trading ProgramData account.
    pub trading_programdata: Pubkey,
    /// Core ProgramData account.
    pub core_programdata: Pubkey,
    /// Claims ProgramData account.
    pub claims_programdata: Pubkey,
    /// Exact ProgramData widths used to finalize the selected profile.
    pub deployment_widths: DirectHotDeploymentWidthsV5,
    /// Transaction payer used by both first-use lifecycle plans.
    pub payer: Pubkey,
    /// Native maker public keys authenticated by detached Ed25519.
    pub makers: [Pubkey; 2],
    /// Current trusted Clock slot encoded into both intents.
    pub clock_slot: u64,
    /// The founded market's geometry.
    ///
    /// One number, not a pair: Product Runtime V2 pins
    /// `outcome_count = cut_count + 2`, so the canonical three-outcome demo is
    /// one cut and the journey's four-outcome market is two. Every
    /// runtime-width record this fixture installs is derived from it, and the
    /// Direct artifacts do not move with it.
    pub geometry: DirectOrdinaryGeometryV3,
    /// Trading interpreter semantic release the activation cache authenticates.
    ///
    /// Decision 0005: it is a seed of the validated-artifact seal, so a Trading
    /// release whose validators differ never reads another release's verdict.
    pub trading_semantic_release: [u8; 32],
    /// The trade executed, which decides how many Custody routes run.
    ///
    /// `DirectTradeScenarioV1::ZERO_FEE` is the historical fixture exactly.
    pub trade: DirectTradeScenarioV1,
}

/// Complete canonical Direct child instruction and owned account declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectHotChainFixtureV5 {
    /// Trading Hot instruction before Registry wrapping.
    pub hot_instruction: Instruction,
    /// Exact distinct fixed prefix required by the write-once seal outer.
    ///
    /// The execution instruction aliases the six seal-authenticated staging
    /// coordinates to their raw coordinates. Seal materialization must retain
    /// both coordinates because it authenticates the Registry finalization
    /// itself before persisting the closure.
    pub capability_seal_accounts: Vec<AccountMeta>,
    /// Exact seller and buyer signed intent preimages.
    pub signed_messages: [[u8; 172]; 2],
    /// All accounts, including externally installed release-waist identities.
    pub accounts: Vec<DirectHotInstallAccountV5>,
    /// Accounts already installed by the parent release-waist ProgramTest.
    pub externally_installed_keys: Vec<Pubkey>,
    /// Exact keys expected to change only after every child succeeds.
    pub rollback_snapshot_keys: Vec<Pubkey>,
    /// Canonical Core Market the request and every child request name.
    pub market: Pubkey,
    /// Canonical mutable root account.
    pub root: Pubkey,
    /// Canonical Claims aggregate.
    pub claims_market: Pubkey,
    /// Canonical seller and buyer Claims Positions.
    pub claims_positions: [Pubkey; 2],
    /// Canonical seller and buyer maker replay accounts.
    pub maker_replays: [Pubkey; 2],
    /// Canonical Custody replay mutated by the delegated transfer.
    pub custody_replay: Pubkey,
    /// Ordered source, destination, and untouched collateral token accounts.
    pub collateral_accounts: [Pubkey; 3],
    /// The four declared Custody routes' caller authorities, in `CUSTODY_ROUTES_V3`
    /// order: seller-terminal, seller-intermediate, fee-continuation, fee-sole.
    pub custody_routes: [CustodyRouteAuthorityV3; 4],
    /// Canonical validated-artifact seal for the selected descriptor closure.
    pub capability_seal: Pubkey,
    /// Exact canonical seal body the on-chain seal outer must produce.
    pub capability_seal_bytes: Vec<u8>,
    /// SHA-256 of the selected descriptor record.
    pub descriptor_digest: [u8; 32],
    /// The Claims caller authority and ITS BUMP, for the campaigns that measure.
    ///
    /// Reported because the search that finds this address is the one search on
    /// the ordinary route that no margin gate subtracts, and a gate cannot
    /// subtract a depth the fixture does not hand it. `None` on the registered
    /// creation chain, which dispatches no Claims child and so searches for no
    /// such authority -- an honest absence rather than a zero that would read as
    /// a first-try hit.
    pub claims_caller_authority: Option<(Pubkey, u8)>,
}

/// One same-validator registered-order creation campaign through generic Hot.
///
/// The two instructions deliberately share one initial account declaration:
/// Sell advances the mutable root first, then Buy authenticates that exact
/// poststate and opens its ordered Custody replay/vault/deposit chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectRegisteredCreationChainFixtureV4 {
    /// Authenticated maker-signed RegisterSell instruction.
    pub sell_hot_instruction: Instruction,
    /// Authenticated maker-signed RegisterBuy instruction, rooted after Sell.
    pub buy_hot_instruction: Instruction,
    /// Exact distinct seal-materialization fixed prefix for Sell.
    pub sell_capability_seal_accounts: Vec<AccountMeta>,
    /// Exact distinct seal-materialization fixed prefix for Buy.
    pub buy_capability_seal_accounts: Vec<AccountMeta>,
    /// Seller then buyer signed intent preimages.
    pub signed_messages: [[u8; 172]; 2],
    /// Complete initial account declaration shared by both transactions.
    pub accounts: Vec<DirectHotInstallAccountV5>,
    /// Release-waist accounts installed by the parent harness.
    pub externally_installed_keys: Vec<Pubkey>,
    /// Exact mutable keys protected by transaction rollback.
    pub rollback_snapshot_keys: Vec<Pubkey>,
    /// Canonical Core Market.
    pub market: Pubkey,
    /// Mutable Direct root advanced from zero to two open maker roots.
    pub root: Pubkey,
    /// Canonical Claims aggregate observed for reserve conservation.
    pub claims_market: Pubkey,
    /// Seller then buyer Claims Positions.
    pub claims_positions: [Pubkey; 2],
    /// Seller then buyer maker-replay PDAs.
    pub maker_replays: [Pubkey; 2],
    /// Seller then buyer registered-record PDAs.
    pub registered_records: [Pubkey; 2],
    /// Market-lifecycle RentCredit shared by both first-use creations.
    pub lifecycle_rent_credit: Pubkey,
    /// RegisterBuy Custody replay opened at revision three.
    pub custody_replay: Pubkey,
    /// RegisterBuy TradingPrincipal vault.
    pub custody_vault: Pubkey,
    /// Buyer source, seller destination, and fee collateral accounts.
    pub collateral_accounts: [Pubkey; 3],
    /// Sell then Buy capability-seal PDAs.
    pub capability_seals: [Pubkey; 2],
    /// Exact expected seal bodies, Sell then Buy.
    pub capability_seal_bytes: [Vec<u8>; 2],
    /// Exact root bytes after Sell and after Buy.
    pub root_poststates: [Vec<u8>; 2],
    /// Exact maker replay bodies after each side's creation.
    pub maker_poststates: [[u8; DIRECT_MAKER_REPLAY_BYTES_V1]; 2],
    /// Exact registered-record bodies after each side's creation.
    pub record_poststates: [[u8; DIRECT_REGISTERED_RECORD_BYTES_V2]; 2],
    /// Claims reserved by the seller record.
    pub reserved_claims: u64,
    /// Collateral reserved in the buyer record and Custody vault.
    pub reserved_collateral: u64,
}

/// Stable refusal from executable Direct fixture construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectHotChainFixtureErrorV5 {
    /// An input identity or arithmetic join was invalid.
    Input,
    /// Artifact construction refused the selected profile geometry.
    Artifact(DirectHotFixtureErrorV5),
    /// A semantic-owner encoder refused fixture state.
    Encoding,
    /// AccountProfile packing found an alias or geometry mismatch.
    Profile,
}

/// Build one executable Direct Hot fixture at the geometry its input states.
/// The bumps a Direct producer mines off chain, derived HERE, from this
/// fixture's own seeds.
///
/// # Why this is a second derivation and not a copy of the builder's block
///
/// `waist::assert_builder_reproduces_hand` compares this fixture against
/// `dclutch-chain-bundle-builder`'s bundle byte for byte, and that comparison
/// is evidence only while the two sides are two AUTHORS. The builder's
/// `mine_bump_hints_v1` reaches its three slots by DECODING account bodies it
/// was handed -- `CoreState` for the Market identity, `CapabilityRootHeaderV1`
/// for the root seeds -- and re-deriving from what it read. This side decodes
/// nothing: it keeps the bump that fell out of the `find_program_address` which
/// PRODUCED each address, from seeds it built itself out of `input`. Two
/// preimages, two walks, one answer -- or the assertion is red and one of them
/// is wrong.
///
/// Writing the builder's bytes across instead would have turned all 52 rows
/// this repaired green while proving exactly nothing, which is the failure this
/// function is shaped to avoid.
///
/// # Which slots are filled, and why the other five stay zero
///
/// `market`, `root` and `child_relay[1]` -- Custody's transfer authority, whose
/// seeds are the Market and the release set -- are the three any off-chain
/// producer can reach; `HotBumpHintsV1`'s own doc and `mine_bump_hints_v1`'s
/// give the reason for each of the rest. A zero slot is correct and merely
/// slower. The whole block is returned rather than three fields because the two
/// authors must agree about the ABSENCES too: a builder that started filling
/// `lifecycle` without this side following would go red here, which is the
/// point.
fn hand_mined_bump_hints_v1(
    input: DirectHotChainInputV5,
    state: &StateFixture,
    capability: &CapabilityFixture,
) -> HotBumpHintsV1 {
    let transfer_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(state.market.to_bytes(), input.release_set).as_slices(),
        &input.custody_program,
    )
    .1;
    HotBumpHintsV1 {
        market: state.market_bump,
        root: capability.root_bump,
        child_relay: [0, transfer_authority],
        ..HotBumpHintsV1::ABSENT
    }
}

/// The two slots the REGISTERED creation route's producer mines, derived here,
/// from this fixture's own seeds.
///
/// The operator's registered path -- `build_direct_hot_request_v4`, which
/// `build_direct_registration_hot_v4` and both registered terminal builders
/// enter through -- fills exactly `market` and `root` and leaves everything
/// else searching, because that function serves every Direct action including
/// the ones with no Custody leg to spend a transfer-authority hint on. This
/// side must therefore fill exactly those two, and
/// `assert_registered_creation_hot_hints_v4` is what says so: it re-derives
/// both by DECODING the bodies this fixture installed, which is the walk the
/// operator's corpus makes, and names the slot when a byte disagrees.
///
/// Two preimages, two walks, one answer. This side keeps the bump that fell out
/// of the `find_program_address` which PRODUCED each address; the assertion's
/// side decodes `CoreState` and `CapabilityRootHeaderV1` out of the installed
/// account and re-derives from what it read.
fn registered_hand_mined_bump_hints_v4(
    state: &StateFixture,
    capability: &CapabilityFixture,
) -> HotBumpHintsV1 {
    HotBumpHintsV1 {
        market: state.market_bump,
        root: capability.root_bump,
        ..HotBumpHintsV1::ABSENT
    }
}

pub fn build_direct_hot_chain_fixture_v5(
    input: DirectHotChainInputV5,
) -> Result<DirectHotChainFixtureV5, DirectHotChainFixtureErrorV5> {
    validate_input(input)?;
    let rent = Rent::default();
    let artifacts = build_direct_hot_artifact_fixture_v5(input.deployment_widths, input.geometry)
        .map_err(DirectHotChainFixtureErrorV5::Artifact)?;
    let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
        .encode();
    let product = product_fixture(input)?;
    let manifest = capability_manifest(input, &artifacts, &config)?;
    let state = market_and_claims(input, &product, &manifest, &rent)?;
    let intents = intents(input, state.market, product.collateral_accounts)?;
    let request = direct_request(input.makers, intents)?;
    let capability = capability_fixture(input, manifest, state.market)?;
    let realm = realm_fixture(
        input,
        product.collateral_accounts,
        state.market,
        capability.buyer_maker,
    )?;
    // Derived ONCE, here, and handed to both consumers: the coordinates the
    // frame installs and the coordinates the fixture reports are the same four
    // values by construction, not by two derivations agreeing.
    let custody_routes =
        custody_route_authorities(input, &product, &state, &capability, &realm, &request)?;
    // Derived here as well as inside the frame builder, and the two are the same
    // call, so the address the fixture REPORTS and the address it INSTALLS
    // cannot drift.
    let claims_caller_authority = claims_caller_authority_v5(input, &product, &state, &request)?;
    let mut logical = logical_accounts(
        input,
        &rent,
        &artifacts,
        &config,
        &product,
        &state,
        &capability,
        &realm,
        &request,
        &custody_routes,
    )?;
    let profile = AccountProfileV2::decode(&artifacts.bundle.account_profile)
        .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
    let runtime = pack_runtime(profile, input.geometry.outcome_count(), &mut logical)?;
    let fixed = fixed_hot_accounts(
        input,
        &rent,
        &artifacts,
        &config,
        &product,
        &state,
        &capability,
    )?;
    let capability_seal = fixed.capability_seal;
    let capability_seal_bytes = fixed.capability_seal_bytes.clone();
    let fixed = fixed.accounts;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).map_err(|_| DirectHotChainFixtureErrorV5::Input)?,
        input.release_set,
        state.market.to_bytes(),
        GENERATION,
        hash(&capability.root_bytes).to_bytes(),
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .with_bump_hints(hand_mined_bump_hints_v1(input, &state, &capability));
    let mut data = Vec::with_capacity(HOT_EXECUTION_ENVELOPE_BYTES_V3 + request.len());
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(&request);
    let mut metas = fixed
        .iter()
        .map(|value| value.meta.clone())
        .collect::<Vec<_>>();
    let capability_seal_accounts = metas.clone();
    alias_sealed_execution_metas(&mut metas)?;
    metas.extend(runtime.iter().skip(5).map(|value| value.meta.clone()));
    let hot_instruction = Instruction {
        program_id: input.trading_program,
        accounts: metas,
        data,
    };
    let mut accounts = fixed
        .into_iter()
        .map(ChainAccount::install)
        .collect::<Vec<_>>();
    for candidate in runtime.into_iter().skip(5) {
        if !accounts.iter().any(|value| value.key == candidate.key) {
            accounts.push(candidate.install());
        }
    }
    let rollback_snapshot_keys = accounts
        .iter()
        .filter(|value| value.snapshot_for_rollback)
        .map(|value| value.key)
        .collect();
    let external_candidates = [
        input.activation_cache,
        input.registry_program,
        input.trading_program,
        input.core_program,
        input.claims_program,
        input.custody_program,
        input.trading_programdata,
        input.core_programdata,
        input.claims_programdata,
        input.payer,
        system_program::ID,
        sysvar::rent::ID,
        sysvar::instructions::ID,
        Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
    ];
    let externally_installed_keys = external_candidates
        .into_iter()
        .filter(|candidate| accounts.iter().any(|value| value.key == *candidate))
        .collect();
    Ok(DirectHotChainFixtureV5 {
        hot_instruction,
        capability_seal_accounts,
        signed_messages: [
            intents[0]
                .signed_preimage()
                .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
            intents[1]
                .signed_preimage()
                .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        ],
        accounts,
        externally_installed_keys,
        rollback_snapshot_keys,
        market: state.market,
        root: capability.root,
        claims_market: state.claims_market,
        claims_positions: [state.positions[0].0, state.positions[1].0],
        maker_replays: [capability.seller_maker, capability.buyer_maker],
        custody_replay: realm.custody_replay,
        collateral_accounts: product.collateral_accounts,
        custody_routes,
        capability_seal,
        capability_seal_bytes,
        descriptor_digest: artifacts.descriptor_id,
        claims_caller_authority: Some(claims_caller_authority),
    })
}

/// Build the canonical same-validator RegisterSell then RegisterBuy campaign.
pub fn build_direct_registered_creation_chain_fixture_v4(
    input: DirectHotChainInputV5,
) -> Result<DirectRegisteredCreationChainFixtureV4, DirectHotChainFixtureErrorV5> {
    validate_input(input)?;
    let rent = Rent::default();
    let config_value = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let config = config_value.encode();
    let product = product_fixture(input)?;
    let artifacts = registered_creation_artifacts_v4(input)?;
    let manifest = capability_manifest_selected(input, artifacts.sell(), &config)?;
    let state = market_and_claims(input, &product, &manifest, &rent)?;
    let capability = capability_fixture(input, manifest, state.market)?;
    let requests =
        registered_creation_requests_v4(input, &rent, config_value, &state, &product, &capability)?;
    let custody = registered_buy_custody_v4(input, &rent, &product, &state, &requests)?;

    let mut sell_logical = registered_creation_logical_accounts_v4(
        input,
        &rent,
        artifacts.sell(),
        &config,
        &product,
        &state,
        &capability,
        &requests,
        Some(&custody),
    )?;
    let mut buy_logical = registered_creation_logical_accounts_v4(
        input,
        &rent,
        artifacts.buy(),
        &config,
        &product,
        &state,
        &capability,
        &requests,
        Some(&custody),
    )?;
    let sell_profile = AccountProfileV2::decode(&artifacts.sell.account_profile)
        .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
    let buy_profile = AccountProfileV2::decode(&artifacts.buy.account_profile)
        .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
    let sell_runtime = pack_runtime(
        sell_profile,
        input.geometry.outcome_count(),
        &mut sell_logical,
    )?;
    let buy_runtime = pack_runtime(
        buy_profile,
        input.geometry.outcome_count(),
        &mut buy_logical,
    )?;
    let sell_fixed = fixed_hot_accounts_selected(
        input,
        &rent,
        artifacts.sell(),
        &config,
        &product,
        &state,
        &capability,
    )?;
    let buy_fixed = fixed_hot_accounts_selected(
        input,
        &rent,
        artifacts.buy(),
        &config,
        &product,
        &state,
        &capability,
    )?;
    let root_poststates = [
        registered_root_poststate_bytes_v4(&capability, requests.root_after_sell)?,
        registered_root_poststate_bytes_v4(&capability, requests.root_after_buy)?,
    ];
    let (sell_hot_instruction, sell_capability_seal_accounts) =
        registered_creation_hot_instruction_v4(
            input,
            &state,
            &capability,
            &capability.root_bytes,
            &requests.requests[0],
            &sell_fixed.accounts,
            &sell_runtime,
        )?;
    let (buy_hot_instruction, buy_capability_seal_accounts) =
        registered_creation_hot_instruction_v4(
            input,
            &state,
            &capability,
            &root_poststates[0],
            &requests.requests[1],
            &buy_fixed.accounts,
            &buy_runtime,
        )?;

    let capability_seals = [sell_fixed.capability_seal, buy_fixed.capability_seal];
    let capability_seal_bytes = [
        sell_fixed.capability_seal_bytes.clone(),
        buy_fixed.capability_seal_bytes.clone(),
    ];
    let mut accounts = Vec::new();
    for candidate in sell_fixed
        .accounts
        .into_iter()
        .chain(sell_runtime.into_iter().skip(5))
        .chain(buy_fixed.accounts.into_iter())
        .chain(buy_runtime.into_iter().skip(5))
    {
        merge_registered_install_account_v4(&mut accounts, candidate)?;
    }
    for candidate in [
        owned(
            &rent,
            state.claims_market,
            input.claims_program,
            state.claims_bytes.clone(),
            true,
        ),
        owned(
            &rent,
            state.positions[0].0,
            input.claims_program,
            state.positions[0].1.clone(),
            true,
        ),
        owned(
            &rent,
            state.positions[1].0,
            input.claims_program,
            state.positions[1].1.clone(),
            true,
        ),
        owned(
            &rent,
            product.collateral_accounts[2],
            custody.realm.token_program,
            token_bytes(
                custody.realm.mint,
                input.payer,
                FEE_COLLATERAL_BALANCE,
                None,
                0,
            )?,
            true,
        ),
    ] {
        merge_registered_install_account_v4(&mut accounts, candidate)?;
    }
    let external_candidates = [
        input.activation_cache,
        input.registry_program,
        input.trading_program,
        input.core_program,
        input.claims_program,
        input.custody_program,
        input.rent_program,
        input.trading_programdata,
        input.core_programdata,
        input.claims_programdata,
        input.payer,
        system_program::ID,
        sysvar::rent::ID,
        sysvar::instructions::ID,
        Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
    ];
    let externally_installed_keys = external_candidates
        .into_iter()
        .filter(|candidate| accounts.iter().any(|value| value.key == *candidate))
        .collect::<Vec<_>>();
    let rollback_snapshot_keys = accounts
        .iter()
        .filter(|value| value.snapshot_for_rollback)
        .map(|value| value.key)
        .collect::<Vec<_>>();
    let lifecycle_rent_credit = lifecycle_rent_credit(
        input.rent_program,
        state.market,
        input.release_set,
        input.payer,
    )?
    .0;
    Ok(DirectRegisteredCreationChainFixtureV4 {
        sell_hot_instruction,
        buy_hot_instruction,
        sell_capability_seal_accounts,
        buy_capability_seal_accounts,
        signed_messages: [
            requests.intents[0]
                .signed_preimage()
                .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
            requests.intents[1]
                .signed_preimage()
                .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        ],
        accounts,
        externally_installed_keys,
        rollback_snapshot_keys,
        market: state.market,
        root: capability.root,
        claims_market: state.claims_market,
        claims_positions: [state.positions[0].0, state.positions[1].0],
        maker_replays: requests.maker_replays,
        registered_records: requests.records,
        lifecycle_rent_credit,
        custody_replay: custody.realm.custody_replay,
        custody_vault: custody.vault,
        collateral_accounts: product.collateral_accounts,
        capability_seals,
        capability_seal_bytes,
        root_poststates,
        maker_poststates: requests.maker_poststates,
        record_poststates: requests.record_poststates,
        reserved_claims: requests.reserved_claims,
        reserved_collateral: requests.reserved_collateral,
    })
}

/// The registered creation fixture's mined block reproduces from the bodies it
/// installed, and when it does not, WHICH SLOT.
///
/// # Why this assertion exists
///
/// The operator's registered path -- `build_direct_hot_request_v4` -- mines
/// `market` and `root` through `dclutch_operator::hot_bump_miner` from the account
/// BODIES its fixed frame supplies. This fixture, which is what the registered
/// campaign actually submits to a real ELF, wrote the all-zero block until
/// today: the same class `8a691ee57` and `e503d5e2a` each repaired one file
/// over, and nothing anywhere compared the two sides. So every registered
/// packet a chain has ever seen carried the pre-hint route while the operator
/// beside it mined two bytes -- and the first thing to notice would have been a
/// CU difference nobody was measuring.
///
/// # Why it is evidence rather than a copy
///
/// Two AUTHORS, one answer. The fixture keeps the bump that fell out of the
/// `find_program_address` which PRODUCED each address, from seeds it built
/// itself. This side decodes `CoreState` and `CapabilityRootHeaderV1` out of
/// the account bodies the fixture installed and re-derives from what it read,
/// under the Core and Trading programs the frame itself names -- which is the
/// operator's corpus exactly. Writing the fixture's two bytes across would turn
/// this green and prove nothing.
///
/// The whole eight-slot block is compared, not two fields, because the two
/// authors must agree about the ABSENCES too: a fixture that started filling
/// `lifecycle` without the operator following would go red here, which is the
/// point. A disagreement is reported at its HOT ENVELOPE offset with the slot
/// named through `HOT_BUMP_HINT_SLOT_NAMES_V1`, so the reader is handed a
/// derivation rather than a byte.
///
/// # Panics
///
/// When either side's Hot instruction is not a canonical Hot envelope, when the
/// frame's Market or root body is not among the installed accounts, or when the
/// two derivations disagree in any of the eight slots.
pub fn assert_registered_creation_hot_hints_v4(fixture: &DirectRegisteredCreationChainFixtureV4) {
    for (side, instruction) in [
        ("RegisterSell", &fixture.sell_hot_instruction),
        ("RegisterBuy", &fixture.buy_hot_instruction),
    ] {
        let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(&instruction.data)
            .expect("canonical registered Hot instruction");
        let key = |coordinate: usize| {
            instruction
                .accounts
                .get(coordinate)
                .unwrap_or_else(|| panic!("{side} fixed frame coordinate {coordinate}"))
                .pubkey
        };
        let body = |coordinate: usize| {
            let wanted = key(coordinate);
            fixture
                .accounts
                .iter()
                .find(|account| account.key == wanted)
                .map(|account| account.account.data.as_slice())
                .unwrap_or_else(|| panic!("{side} installed account for coordinate {coordinate}"))
        };
        let mined = mine_hot_bump_hints_v1(&HotBumpCorpusV1 {
            market_key: key(HOT_MARKET_ACCOUNT_V3),
            market_data: body(HOT_MARKET_ACCOUNT_V3),
            root_data: body(HOT_ROOT_ACCOUNT_V3),
            core_program: key(HOT_CORE_PROGRAM_ACCOUNT_V3),
            trading_program: key(HOT_TRADING_PROGRAM_ACCOUNT_V3),
            // The operator's registered path leaves this searching for every
            // Direct action, so a fixture that filled it would disagree with
            // the producer it stands beside. See
            // `registered_hand_mined_bump_hints_v4`.
            custody_program: None,
            release_set: envelope.release_set(),
        });
        let differing = envelope
            .bump_hints()
            .to_bytes()
            .into_iter()
            .zip(mined.to_bytes())
            .enumerate()
            .filter(|(_, (fixture_byte, operator_byte))| fixture_byte != operator_byte)
            .map(|(slot, (fixture_byte, operator_byte))| {
                let offset = HOT_BUMP_HINTS_OFFSET_V1 + slot;
                let name = hot_bump_hint_slot_name_v1(offset).unwrap_or("out of block");
                format!("{offset} (HotBumpHintsV1::{name}) fixture {fixture_byte} operator {operator_byte}")
            })
            .collect::<Vec<_>>();
        assert!(
            differing.is_empty(),
            "{side} hand fixture and operator corpus disagree in {} of {} mined bump hints: {}",
            differing.len(),
            HOT_BUMP_HINT_COUNT_V1,
            differing.join("; ")
        );
        assert_ne!(
            envelope.bump_hints(),
            HotBumpHintsV1::ABSENT,
            "{side} mined nothing at all, so both sides agree on an all-zero block \
             and this assertion proves only that neither producer ran",
        );
    }
}

fn registered_creation_hot_instruction_v4(
    input: DirectHotChainInputV5,
    state: &StateFixture,
    capability: &CapabilityFixture,
    root_prestate: &[u8],
    request: &[u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3],
    fixed: &[ChainAccount],
    runtime: &[ChainAccount],
) -> Result<(Instruction, Vec<AccountMeta>), DirectHotChainFixtureErrorV5> {
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).map_err(|_| DirectHotChainFixtureErrorV5::Input)?,
        input.release_set,
        state.market.to_bytes(),
        GENERATION,
        hash(root_prestate).to_bytes(),
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .with_bump_hints(registered_hand_mined_bump_hints_v4(state, capability));
    let mut data = Vec::with_capacity(HOT_EXECUTION_ENVELOPE_BYTES_V3 + request.len());
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(request);
    let mut metas = fixed
        .iter()
        .map(|value| value.meta.clone())
        .collect::<Vec<_>>();
    let seal_accounts = metas.clone();
    // NOT aliased, and that is a rule of the executing program rather than a
    // fixture preference. `hot_v3` computes
    // `frame.uses_sealed_execution_aliases()` and requires it to EQUAL
    // `selected_kind == DIRECT_SUCCESSOR_KIND_ID_V3 && selected_action ==
    // InlineOrdinary`, refusing `TradingSbfError::Content` (0x4003) either way
    // round. The entitlement is packet relief for the one action that needed
    // it -- the ordinary continuation sat at 1,198 of 1,232 bytes and the six
    // aliases bought it six lookup indexes -- and the seal is what makes the
    // staging coordinate redundant there. Every other action, this one
    // included, keeps the fully distinct frame.
    //
    // Measured 2026-09-01 (lane DIRECT-SELLBUY): this builder aliased
    // unconditionally, copied from the ordinary builder, so the FIRST registered
    // Sell ever submitted to a real ELF refused `Content` at 117,613 CU in the
    // band between the `root-product` and `artifacts-strategy-effect`
    // checkpoints, before any child CPI. Nothing caught it because no registered
    // creation had ever executed on a chain.
    metas.extend(runtime.iter().skip(5).map(|value| value.meta.clone()));
    Ok((
        Instruction {
            program_id: input.trading_program,
            accounts: metas,
            data,
        },
        seal_accounts,
    ))
}

fn registered_root_poststate_bytes_v4(
    capability: &CapabilityFixture,
    state: DirectRootStateV1,
) -> Result<Vec<u8>, DirectHotChainFixtureErrorV5> {
    let header = capability
        .root_bytes
        .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    let mut output = Vec::with_capacity(
        CAPABILITY_ROOT_HEADER_BYTES_V1 + dclutch_trading::successor::DIRECT_ROOT_STATE_BYTES_V1,
    );
    output.extend_from_slice(header);
    output.extend_from_slice(&state.encode());
    Ok(output)
}

fn merge_registered_install_account_v4(
    accounts: &mut Vec<DirectHotInstallAccountV5>,
    candidate: ChainAccount,
) -> Result<(), DirectHotChainFixtureErrorV5> {
    if let Some(existing) = accounts.iter_mut().find(|value| value.key == candidate.key) {
        if existing.account != candidate.account {
            return Err(DirectHotChainFixtureErrorV5::Profile);
        }
        existing.snapshot_for_rollback |= candidate.snapshot;
    } else {
        accounts.push(candidate.install());
    }
    Ok(())
}

/// Apply the seal-backed alias shape to a built fixed frame.
///
/// The six pairs are the ABI's, read from
/// [`SEALED_EXECUTION_FIXED_ALIASES_V3`] rather than restated. This was the
/// FOURTH hand-written copy of that table -- `aa72e3a09` retired the executor's
/// and the operator's and said three authorities had become one, and two more
/// were in this crate, out of the reach of that commit's grep.
fn alias_sealed_execution_metas(
    metas: &mut [AccountMeta],
) -> Result<(), DirectHotChainFixtureErrorV5> {
    for (raw, staging) in SEALED_EXECUTION_FIXED_ALIASES_V3 {
        let raw = metas
            .get(raw)
            .ok_or(DirectHotChainFixtureErrorV5::Profile)?
            .clone();
        let staging = metas
            .get_mut(staging)
            .ok_or(DirectHotChainFixtureErrorV5::Profile)?;
        if raw.is_signer || raw.is_writable || staging.is_signer || staging.is_writable {
            return Err(DirectHotChainFixtureErrorV5::Profile);
        }
        *staging = raw;
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct ChainAccount {
    key: Pubkey,
    account: Account,
    meta: AccountMeta,
    snapshot: bool,
}

impl ChainAccount {
    fn install(self) -> DirectHotInstallAccountV5 {
        DirectHotInstallAccountV5 {
            key: self.key,
            account: self.account,
            snapshot_for_rollback: self.snapshot,
        }
    }
}

#[derive(Clone, Debug)]
struct Finalized {
    raw: Pubkey,
    staging: Pubkey,
    bytes: Vec<u8>,
    digest: [u8; 32],
    schema: [u8; 32],
    owner: Pubkey,
}

struct ProductFixture {
    product_id: [u8; 32],
    semantic_basis: [u8; 32],
    product: Finalized,
    domain: Finalized,
    portfolio: Finalized,
    basis: Finalized,
    collateral_accounts: [Pubkey; 3],
}

struct StateFixture {
    market: Pubkey,
    /// The canonical bump of `market`, kept from the derivation that produced
    /// the address rather than searched for a second time. It is what a
    /// founding writes into `StateBumpsV1` and what the hot envelope's
    /// `HotBumpHintsV1::market` slot carries.
    market_bump: u8,
    core_bytes: Vec<u8>,
    claims_market: Pubkey,
    claims_bytes: Vec<u8>,
    positions: [(Pubkey, Vec<u8>); 2],
}

struct CapabilityFixture {
    root: Pubkey,
    /// The canonical bump of `root`, kept from the same derivation. See
    /// `StateFixture::market_bump`.
    root_bump: u8,
    root_bytes: Vec<u8>,
    seller_maker: Pubkey,
    buyer_maker: Pubkey,
    manifest: Vec<u8>,
}

struct CapabilityManifestFixture {
    bytes: Vec<u8>,
    selection: CapabilityExecutionSelectionV1,
    /// Exactly what the on-chain activation would have recorded in the root:
    /// the canonical bumps of the manifest and config records.
    record_bumps: SelectedRecordBumpsV1,
}

/// One action-selected Hot closure borrowed from its canonical artifact
/// producer. The ordinary and registered families share this transport shape;
/// the concrete bundle types remain the semantic owners of their bytes.
#[derive(Clone, Copy)]
struct SelectedHotArtifactsV5<'a> {
    action: DirectExecutionActionV3,
    program_set: &'a [u8],
    program_set_id: [u8; 32],
    descriptor: &'a [u8],
    descriptor_id: [u8; 32],
    account_profile: &'a [u8],
    lifecycle_policy: &'a [u8],
    request_profile: &'a [u8],
    transition: &'a [u8],
    strategy: &'a [u8],
    effect: &'a [u8],
}

fn selected_ordinary_artifacts(
    artifacts: &DirectHotArtifactFixtureV5,
) -> SelectedHotArtifactsV5<'_> {
    SelectedHotArtifactsV5 {
        action: DirectExecutionActionV3::InlineOrdinary,
        program_set: &artifacts.program_set,
        program_set_id: artifacts.program_set_id,
        descriptor: &artifacts.bundle.descriptor,
        descriptor_id: artifacts.descriptor_id,
        account_profile: &artifacts.bundle.account_profile,
        lifecycle_policy: &artifacts.bundle.lifecycle_policy,
        request_profile: &artifacts.bundle.request_profile,
        transition: &artifacts.bundle.transition,
        strategy: &artifacts.bundle.strategy,
        effect: &artifacts.bundle.effect,
    }
}

struct DirectRegisteredCreationArtifactsV4 {
    sell: DirectRegisterSellHotBundleV4,
    buy: DirectRegisterBuyHotBundleV4,
    program_set: Vec<u8>,
    program_set_id: [u8; 32],
}

impl DirectRegisteredCreationArtifactsV4 {
    fn sell(&self) -> SelectedHotArtifactsV5<'_> {
        SelectedHotArtifactsV5 {
            action: DirectExecutionActionV3::RegisterSell,
            program_set: &self.program_set,
            program_set_id: self.program_set_id,
            descriptor: &self.sell.descriptor,
            descriptor_id: hash(&self.sell.descriptor).to_bytes(),
            account_profile: &self.sell.account_profile,
            lifecycle_policy: &self.sell.lifecycle_policy,
            request_profile: &self.sell.request_profile,
            transition: &self.sell.transition,
            strategy: &self.sell.strategy,
            effect: &self.sell.effect,
        }
    }

    fn buy(&self) -> SelectedHotArtifactsV5<'_> {
        SelectedHotArtifactsV5 {
            action: DirectExecutionActionV3::RegisterBuy,
            program_set: &self.program_set,
            program_set_id: self.program_set_id,
            descriptor: &self.buy.descriptor,
            descriptor_id: hash(&self.buy.descriptor).to_bytes(),
            account_profile: &self.buy.account_profile,
            lifecycle_policy: &self.buy.lifecycle_policy,
            request_profile: &self.buy.request_profile,
            transition: &self.buy.transition,
            strategy: &self.buy.strategy,
            effect: &self.buy.effect,
        }
    }
}

fn registered_creation_artifacts_v4(
    input: DirectHotChainInputV5,
) -> Result<DirectRegisteredCreationArtifactsV4, DirectHotChainFixtureErrorV5> {
    let common = registered_creation_common_lengths_v4(input)?;
    let mut sell_lengths = common[..usize::from(DIRECT_REGISTER_SELL_FIXED_ACCOUNTS_V4)].to_vec();
    *sell_lengths
        .get_mut(usize::from(DIRECT_REGISTER_SELL_COLLATERAL_ACCOUNT_V4))
        .ok_or(DirectHotChainFixtureErrorV5::Profile)? =
        u32::try_from(SplAccount::LEN).map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    // ONE width for both sides, because there is now one policy. The lifecycle
    // policy belongs to the ROOT, not to an action, so it names the Buy's
    // Custody quotes even when the Sell is the side being built -- and if these
    // two ever disagreed the sides would emit different policy bytes and wall B
    // would silently come back. Named once so they cannot.
    let child_rent_widths = DirectRegisteredCreationChildRentWidthsV4 {
        custody_vault: u32::try_from(SplAccount::LEN)
            .map_err(|_| DirectHotChainFixtureErrorV5::Input)?,
    };
    let sell = build_direct_register_sell_hot_bundle_v4(DirectRegisterSellHotBundleInputV4 {
        account_profile: DirectRegisterBuyAccountProfileInputV4 {
            logical_data_lengths: &sell_lengths,
        },
        child_rent_widths,
        capacity_profile: DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5,
    })
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;

    let buy_lengths = registered_creation_buy_lengths_v4(input, common)?;
    let buy = build_direct_register_buy_hot_bundle_v4(DirectRegisterBuyHotBundleInputV4 {
        account_profile: DirectRegisterBuyAccountProfileInputV4 {
            logical_data_lengths: &buy_lengths,
        },
        child_rent_widths,
        capacity_profile: DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5,
    })
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;

    let entries = [
        validate_direct_register_sell_capability_v4(&sell, DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5)
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        validate_direct_register_buy_capability_v4(&buy, DIRECT_HOT_FIXTURE_CAPACITY_PROFILE_V5)
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
    ];
    let width = encoded_direct_program_set_bytes_v4(entries.len())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut scratch = vec![0_u8; width];
    let mut program_set = vec![0_u8; width];
    encode_direct_program_set_v2_atomic(&entries, &mut scratch, &mut program_set)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    Ok(DirectRegisteredCreationArtifactsV4 {
        sell,
        buy,
        program_set_id: hash(&program_set).to_bytes(),
        program_set,
    })
}

fn registered_creation_common_lengths_v4(
    input: DirectHotChainInputV5,
) -> Result<[u32; 56], DirectHotChainFixtureErrorV5> {
    let mut lengths = [0_u32; 56];
    lengths[0] = u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1)
        .ok()
        .and_then(|header| {
            header.checked_add(
                u32::try_from(dclutch_trading::successor::DIRECT_ROOT_STATE_BYTES_V1).ok()?,
            )
        })
        .ok_or(DirectHotChainFixtureErrorV5::Input)?;
    lengths[1] = u32::try_from(dclutch_trading::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[2] =
        u32::try_from(PRODUCT_RECORD_BYTES_V2).map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[3] = input
        .geometry
        .portfolio_record_bytes()
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[4] = u32::try_from(dclutch_product::payoff::runtime_v3::BASIS_HEADER_BYTES_V3)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[5] = u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[7] = u32::try_from(dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[8] = u32::try_from(DIRECT_REGISTERED_RECORD_BYTES_V2)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    Ok(lengths)
}

fn registered_creation_buy_lengths_v4(
    input: DirectHotChainInputV5,
    mut lengths: [u32; 56],
) -> Result<[u32; 56], DirectHotChainFixtureErrorV5> {
    let loader_bytes = u32::try_from(dclutch_registry::svm::LOADER_V3_PROGRAM_BYTES)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[10] = loader_bytes;
    lengths[13] = u32::try_from(dclutch_market::STATE_BYTES)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[14] = u32::try_from(dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[15] = loader_bytes;
    lengths[16] = loader_bytes;
    lengths[17] = input.deployment_widths.trading_programdata_bytes;
    lengths[18] = u32::try_from(dclutch_market::realm::REALM_BYTES)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[23] = 17;
    lengths[33] =
        u32::try_from(CUSTODY_REPLAY_BYTES_V1).map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[34] = u32::try_from(SplMint::LEN).map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[36] = 0;
    lengths[50] =
        u32::try_from(SplAccount::LEN).map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    lengths[usize::from(DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4)] = loader_bytes;
    for (account, representative) in [
        (9_usize, 6_usize),
        (21, 6),
        (24, 7),
        (22, 11),
        (26, 13),
        (27, 14),
        (28, 15),
        (29, 16),
        (30, 17),
        (31, 18),
        (32, 19),
        (33, 20),
        (38, 6),
        (39, 11),
        (40, 23),
        (42, 13),
        (43, 14),
        (44, 15),
        (45, 16),
        (46, 17),
        (47, 18),
        (48, 19),
        (49, 20),
        (50, 34),
        (52, 35),
        (53, 36),
        (54, 37),
    ] {
        lengths[account] = lengths[representative];
    }
    Ok(lengths)
}

struct RealmFixture {
    realm: Finalized,
    mint: Pubkey,
    token_program: Pubkey,
    custody_replay: Pubkey,
    custody_replay_bytes: Vec<u8>,
    custody_authority: Pubkey,
}

fn validate_input(input: DirectHotChainInputV5) -> Result<(), DirectHotChainFixtureErrorV5> {
    let identities = [
        input.registry_program,
        input.trading_program,
        input.core_program,
        input.claims_program,
        input.custody_program,
        input.rent_program,
        input.activation_cache,
        input.trading_programdata,
        input.core_programdata,
        input.claims_programdata,
        input.payer,
        input.makers[0],
        input.makers[1],
    ];
    if input.release_set == [0; 32]
        || identities.iter().any(|value| *value == Pubkey::default())
        || input.makers[0] == input.makers[1]
    {
        return Err(DirectHotChainFixtureErrorV5::Input);
    }
    Ok(())
}

fn product_fixture(
    input: DirectHotChainInputV5,
) -> Result<ProductFixture, DirectHotChainFixtureErrorV5> {
    let product_id = product_content([0x51; 32])?;
    let coordinate_domain = product_content([0x52; 32])?;
    let result_unit = product_content([0x53; 32])?;
    let provisional_input = BasisInputV3 {
        kind: BasisKindV3::CategoricalQ1,
        product_id: product_id.to_bytes(),
        result_domain_id: [0x54; 32],
        coordinate_domain_id: coordinate_domain.to_bytes(),
        result_unit_id: result_unit.to_bytes(),
        evaluator_release_id: [0x55; 32],
        basis_width: input.geometry.outcome_count(),
        payout_scale: 1,
        knot_denominator: 1,
        knots: &[],
        terms: &[],
        failure_payouts: &[],
        // Exempt by proof: degree 0 and 1 need no price gate,
        // and a digest offered alongside one is refused.
        price_gate_certificate_digest: [0_u8; 32],
    };
    let outcomes = usize::try_from(input.geometry.outcome_count())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let basis_bytes = basis_record_bytes_v3(BasisKindV3::CategoricalQ1, outcomes, 0, 0)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut provisional = vec![0_u8; basis_bytes];
    compile_basis_v3(provisional_input, &mut provisional)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    let semantic = semantic_basis_preimage_v3(&provisional)
        .map_err(|_| DirectHotChainFixtureErrorV5::Input)?;
    let semantic_basis = hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes();
    // Strictly increasing cut numerators, one fewer than the ordinary regions
    // and two fewer than the outcomes. A CategoricalQ1 portfolio weights every
    // outcome equally, so the coefficients are one per outcome.
    let cuts: Vec<i128> = (0..i128::from(input.geometry.cut_count())).collect();
    let coefficients = vec![1_u64; outcomes];
    let mut product_bytes = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain_bytes = vec![
        0_u8;
        result_domain_record_bytes(cuts.len())
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    ];
    let mut portfolio_bytes = vec![
        0_u8;
        portfolio_record_bytes(coefficients.len())
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    ];
    compile_product_records_v2(
        input.registry_program,
        ProductCompilationInputV2 {
            product_id,
            coordinate_domain_id: coordinate_domain,
            result_unit_id: result_unit,
            claim_basis_id: product_content([0x56; 32])?,
            liability_basis_id: product_content(semantic_basis)?,
            representation_release_id: product_content([0x57; 32])?,
            mapping_release_id: product_content([0x58; 32])?,
            cut_denominator: 1,
            cuts: &cuts,
            portfolio_denominator: 1,
            coefficients: &coefficients,
        },
        &mut product_bytes,
        &mut domain_bytes,
        &mut portfolio_bytes,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
    let product = finalized(
        input.registry_program,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        product_bytes,
    );
    let domain = finalized(
        input.registry_program,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        domain_bytes,
    );
    let portfolio = finalized(
        input.registry_program,
        PORTFOLIO_SCHEMA_ID_V2,
        portfolio_bytes,
    );
    let mut linked_basis = vec![0_u8; basis_bytes];
    compile_basis_v3(
        BasisInputV3 {
            result_domain_id: domain.digest,
            ..provisional_input
        },
        &mut linked_basis,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let basis = finalized(
        input.registry_program,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        linked_basis,
    );
    Ok(ProductFixture {
        product_id: product_id.to_bytes(),
        semantic_basis,
        product,
        domain,
        portfolio,
        basis,
        collateral_accounts: [key(0xa1), key(0xa2), key(0xa3)],
    })
}

fn market_and_claims(
    input: DirectHotChainInputV5,
    product: &ProductFixture,
    manifest: &CapabilityManifestFixture,
    rent: &Rent,
) -> Result<StateFixture, DirectHotChainFixtureErrorV5> {
    let realm = realm_record(product.collateral_accounts)?;
    let realm_id = hash(&realm).to_bytes();
    let provisional = MarketIdentity {
        market_id: core_identity([0x61; 32])?,
        realm_id: core_identity(realm_id)?,
        product_record: core_identity(product.product.digest)?,
        product_id: core_identity(product.product_id)?,
        resolution_policy: core_identity([0x62; 32])?,
        capability_manifest: core_identity(hash(&manifest.bytes).to_bytes())?,
        selected_release_set: core_identity(input.release_set)?,
        registry_program: core_identity(input.registry_program.to_bytes())?,
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(provisional).as_slices(),
        &input.core_program,
    )
    .0;
    let identity = MarketIdentity {
        market_id: core_identity(market.to_bytes())?,
        ..provisional
    };
    // The bumps a real founding writes into this state, derived the way
    // `plan_found` derives them (`programs/dclutch-core-sbf/src/found.rs`) and
    // not the way a fixture might find convenient.
    //
    // The Market bump: Core derives `(expected_market, bump)` from
    // `MarketCoreStateSeedsV2::new(market_identity)` on the FINAL identity --
    // the one whose `market_id` is the account it is about to create -- and
    // records `StateBumpsV1::record(bump)`. `MarketCoreStateSeedsV2` projects
    // the identity EXCLUDING the derived address, so this is necessarily the
    // same pair the provisional derivation above produced. Derived again from
    // `identity` anyway, because the founding's input is the thing being
    // mirrored, and compared, because the day those two stop agreeing this
    // fixture is staging a state no founding can write.
    let (founding_market, market_bump) = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &input.core_program,
    );
    if founding_market != market {
        return Err(DirectHotChainFixtureErrorV5::Encoding);
    }
    // The realm pair: `authenticate_references` hands `plan_found` the raw and
    // staging bumps that `authenticate_content_addressed_record` derived while
    // authenticating the Realm record, under the Registry, at
    // `REALM_SCHEMA_RELEASE_ID_V1` and the hash of the record's own bytes.
    // `record_bumps_v1` is that derivation, and `realm_id` is that hash.
    let realm_record_bumps =
        record_bumps_v1(input.registry_program, REALM_SCHEMA_RELEASE_ID_V1, realm_id);
    // The Product graph's four record pairs, in the reader's walk order, which
    // is what `authenticate_founding_product_basis_v3` hands `plan_found` and
    // `ProductGraphBumpsV1::record` carries. Same rule as the realm pair above:
    // an unrecorded tail is the pre-hint route, so a fixture that leaves it
    // absent measures a market no founding produces.
    let product_graph_bumps = {
        let mut bumps = [0_u8; PRODUCT_GRAPH_BUMP_COUNT];
        for (slot, (schema, digest)) in [
            (PRODUCT_RECORD_SCHEMA_ID_V2, product.product.digest),
            (RESULT_DOMAIN_SCHEMA_ID_V2, product.domain.digest),
            (PORTFOLIO_SCHEMA_ID_V2, product.portfolio.digest),
            (GRADED_BASIS_RECORD_SCHEMA_ID_V3, product.basis.digest),
        ]
        .into_iter()
        .enumerate()
        {
            let (raw, staging) = record_bumps_v1(input.registry_program, schema, digest);
            if let Some(cell) = bumps.get_mut(slot * 2) {
                *cell = raw;
            }
            if let Some(cell) = bumps.get_mut(slot * 2 + 1) {
                *cell = staging;
            }
        }
        bumps
    };
    // One credit per Market lifecycle, and the Market is its own PDA seed.
    let rent_credit =
        lifecycle_rent_credit(input.rent_program, market, input.release_set, input.payer)?;
    let core_bytes = CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: core_identity(rent_credit.0.to_bytes())?,
        terminal_receipt: None,
        // What a founded Market's account actually holds. It was
        // `StateBumpsV1::UNRECORDED` from `e93fe5e9`, which added the field and
        // kept this file compiling, until 2026-08-30 -- and an unrecorded tail
        // is not a neutral default, it is the pre-`a0cba859` route: all three
        // Market readers (Trading `hot_v3`, Claims `sparse_native_transfer_v1`,
        // Custody `lib`) and Custody's realm raw/staging pair take their `None`
        // arm and SEARCH. Every compute measurement taken on this fixture was
        // therefore measuring a market no widened founding produces, and the
        // carry looked free because it was never exercised.
        //
        // A wrong bump here does not go unnoticed: each reader reproduces the
        // address with `create_program_address` and compares it against the
        // account it was handed, so a wrong bump refuses. See
        // `direct_hot_top_level_margin_gate.rs`, which stages a deliberately
        // wrong one and asserts the refusal.
        bumps: StateBumpsV1 {
            market: StateBumpsV1::record(market_bump),
            realm_raw_record: StateBumpsV1::record(realm_record_bumps.0),
            realm_staging_record: StateBumpsV1::record(realm_record_bumps.1),
            product_graph: ProductGraphBumpsV1::record(product_graph_bumps),
        },
    }
    .encode()
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .to_vec();
    let (claims_market, claims_market_bump) = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
        &input.claims_program,
    );
    let claims_input = LiabilityBasisMarketInputV2 {
        revision: CLAIMS_MARKET_REVISION,
        logical_market: market.to_bytes(),
        release_set: input.release_set,
        registry_program: input.registry_program.to_bytes(),
        product_instance_id: product.product_id,
        basis_id: product.semantic_basis,
        realm_id,
        custody_context: [0x64; 32],
        generation: GENERATION,
    };
    let outcomes = usize::try_from(input.geometry.outcome_count())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut claims_bytes = vec![0_u8; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + outcomes * 8];
    let supplies = vec![input.trade.claim_supply; outcomes];
    encode_liability_basis_market_into_v2(claims_input, &supplies, &mut claims_bytes)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    // The Claims founding route records these bumps when it creates the
    // accounts, and the hot route's readers reproduce the addresses from them.
    // A fixture that left them zero would stage accounts no deployment produces
    // and would measure a route nobody runs.
    put_liability_basis_market_bump_v2(&mut claims_bytes, claims_market_bump)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    // The seller holds the whole supply of the traded outcome, which is
    // coordinate zero of the Product tail at every geometry; the buyer holds
    // nothing until this transition lands.
    let mut seller_balances = vec![0_u64; outcomes];
    *seller_balances
        .first_mut()
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)? = input.trade.claim_supply;
    let buyer_balances = vec![0_u64; outcomes];
    let seller = position(
        input.claims_program,
        claims_market,
        input.makers[0],
        product.semantic_basis,
        SELLER_POSITION_REVISION,
        &seller_balances,
    )?;
    let buyer = position(
        input.claims_program,
        claims_market,
        input.makers[1],
        product.semantic_basis,
        BUYER_POSITION_REVISION,
        &buyer_balances,
    )?;
    let _ = rent;
    Ok(StateFixture {
        market,
        market_bump,
        core_bytes,
        claims_market,
        claims_bytes,
        positions: [seller, buyer],
    })
}

fn position(
    claims_program: Pubkey,
    claims_market: Pubkey,
    owner: Pubkey,
    basis: [u8; 32],
    revision: u64,
    balances: &[u64],
) -> Result<(Pubkey, Vec<u8>), DirectHotChainFixtureErrorV5> {
    let seeds = ProtocolPositionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let (account, bump) = Pubkey::find_program_address(&seeds.as_slices(), &claims_program);
    let mut bytes = vec![0_u8; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + balances.len() * 8];
    encode_liability_basis_position_into_v2(
        LiabilityBasisPositionInputV2 {
            revision,
            market_account: claims_market.to_bytes(),
            owner: owner.to_bytes(),
            basis_id: basis,
        },
        balances,
        &mut bytes,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    put_liability_basis_position_bump_v2(&mut bytes, bump)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    Ok((account, bytes))
}

fn intents(
    input: DirectHotChainInputV5,
    market: Pubkey,
    collateral: [Pubkey; 3],
) -> Result<[CompactIntentV2; 2], DirectHotChainFixtureErrorV5> {
    Ok([
        CompactIntentV2 {
            side: 0,
            lifecycle: 0,
            outcome: 0,
            market: market.to_bytes(),
            generation: GENERATION,
            nonce: 0,
            valid_from: input.clock_slot.saturating_sub(1),
            valid_through: input.clock_slot.saturating_add(1),
            maximum_fill: input.trade.fill,
            limit_price: input.trade.execution_price,
            fee_basis_points: FEE_BPS,
            collateral_account: collateral[1].to_bytes(),
        },
        CompactIntentV2 {
            side: 1,
            lifecycle: 0,
            outcome: 0,
            market: market.to_bytes(),
            generation: GENERATION,
            nonce: 0,
            valid_from: input.clock_slot.saturating_sub(1),
            valid_through: input.clock_slot.saturating_add(1),
            maximum_fill: input.trade.fill,
            limit_price: input.trade.execution_price,
            fee_basis_points: FEE_BPS,
            collateral_account: collateral[0].to_bytes(),
        },
    ])
}

fn direct_request(
    makers: [Pubkey; 2],
    intents: [CompactIntentV2; 2],
) -> Result<[u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3], DirectHotChainFixtureErrorV5> {
    let mut output = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3];
    let body = encode_header_v3(DirectExecutionActionV3::InlineOrdinary, &mut output)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let seller = intents[0]
        .signed_preimage()
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let buyer = intents[1]
        .signed_preimage()
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    put(body, 0, &makers[0].to_bytes())?;
    put(body, 32, &seller)?;
    put(body, 204, &makers[1].to_bytes())?;
    put(body, 236, &buyer)?;
    // Read off the intents rather than off the fixture's constants: the fill
    // and price the request carries and the fill and price the makers SIGNED
    // are then the same values by construction, and a scenario cannot move one
    // without moving the other.
    put(body, 408, &intents[0].maximum_fill.to_le_bytes())?;
    put(body, 416, &intents[0].limit_price.to_le_bytes())?;
    Ok(output)
}

fn registered_intents(
    input: DirectHotChainInputV5,
    market: Pubkey,
    collateral: [Pubkey; 3],
) -> [CompactIntentV2; 2] {
    [
        CompactIntentV2 {
            side: 0,
            lifecycle: 2,
            outcome: 0,
            market: market.to_bytes(),
            generation: GENERATION,
            nonce: 0,
            valid_from: input.clock_slot.saturating_sub(1),
            valid_through: input.clock_slot.saturating_add(4),
            maximum_fill: input.trade.fill,
            limit_price: input.trade.execution_price,
            fee_basis_points: FEE_BPS,
            collateral_account: collateral[1].to_bytes(),
        },
        CompactIntentV2 {
            side: 1,
            lifecycle: 2,
            outcome: 0,
            market: market.to_bytes(),
            generation: GENERATION,
            nonce: 0,
            valid_from: input.clock_slot.saturating_sub(1),
            valid_through: input.clock_slot.saturating_add(4),
            maximum_fill: input.trade.fill,
            limit_price: input.trade.execution_price,
            fee_basis_points: FEE_BPS,
            collateral_account: collateral[0].to_bytes(),
        },
    ]
}

struct RegisteredCreationRequestsV4 {
    intents: [CompactIntentV2; 2],
    requests: [[u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3]; 2],
    maker_replays: [Pubkey; 2],
    records: [Pubkey; 2],
    root_after_sell: DirectRootStateV1,
    root_after_buy: DirectRootStateV1,
    maker_poststates: [[u8; DIRECT_MAKER_REPLAY_BYTES_V1]; 2],
    record_poststates: [[u8; DIRECT_REGISTERED_RECORD_BYTES_V2]; 2],
    reserved_claims: u64,
    reserved_collateral: u64,
}

fn registered_creation_requests_v4(
    input: DirectHotChainInputV5,
    rent: &Rent,
    config: DirectExecutionConfigV1,
    state: &StateFixture,
    product: &ProductFixture,
    capability: &CapabilityFixture,
) -> Result<RegisteredCreationRequestsV4, DirectHotChainFixtureErrorV5> {
    let intents = registered_intents(input, state.market, product.collateral_accounts);
    let (rent_credit, _) = lifecycle_rent_credit(
        input.rent_program,
        state.market,
        input.release_set,
        input.payer,
    )?;
    let maker_principal = rent.minimum_balance(DIRECT_MAKER_REPLAY_BYTES_V1);
    let record_principal = rent.minimum_balance(DIRECT_REGISTERED_RECORD_BYTES_V2);
    if maker_principal == 0 || record_principal == 0 {
        return Err(DirectHotChainFixtureErrorV5::Encoding);
    }
    let mut requests = [[0_u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3]; 2];
    let mut maker_replays = [Pubkey::default(); 2];
    let mut records = [Pubkey::default(); 2];
    let mut maker_poststates = [[0_u8; DIRECT_MAKER_REPLAY_BYTES_V1]; 2];
    let mut record_poststates = [[0_u8; DIRECT_REGISTERED_RECORD_BYTES_V2]; 2];
    let mut root = DirectRootStateV1::new();
    let mut root_after_sell = None;
    let mut reserved_claims = 0_u64;
    let mut reserved_collateral = 0_u64;
    for (index, action) in [
        DirectExecutionActionV3::RegisterSell,
        DirectExecutionActionV3::RegisterBuy,
    ]
    .into_iter()
    .enumerate()
    {
        let maker = *input
            .makers
            .get(index)
            .ok_or(DirectHotChainFixtureErrorV5::Input)?;
        let intent = *intents
            .get(index)
            .ok_or(DirectHotChainFixtureErrorV5::Input)?;
        let authenticated =
            AuthenticatedCompactIntentV2::from_adjacent_ed25519(maker.to_bytes(), intent)
                .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        let maker_seeds = MakerReplaySeedsV1::new(
            DirectCoordinatesV1::new(state.market.to_bytes(), GENERATION)
                .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
            maker.to_bytes(),
        )
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        let (maker_replay, maker_bump) =
            Pubkey::find_program_address(&maker_seeds.as_slices(), &input.trading_program);
        let record_seeds = RegisteredIntentSeedsV2::new(authenticated)
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        let (record, record_bump) =
            Pubkey::find_program_address(&record_seeds.as_slices(), &input.trading_program);
        let created = register_intent_v2(
            root,
            MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(maker_bump, 0)),
            authenticated,
            config,
            input.geometry.outcome_count(),
            Some(MakerReplayFirstUseV1 {
                rent_owner: input.payer.to_bytes(),
                rent_principal: maker_principal,
            }),
            RegisteredRecordFirstUseV2 {
                bump: record_bump,
                observed_lamports: 0,
                rent_owner: input.payer.to_bytes(),
                rent_principal: record_principal,
            },
        )
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        let request = DirectRegistrationRequestV3 {
            participant: DirectSignedParticipantV3 {
                maker: maker.to_bytes(),
                intent,
            },
            maker_rent_credit: rent_credit.to_bytes(),
            record_rent_credit: rent_credit.to_bytes(),
            maker_rent_principal: maker_principal,
            record_rent_principal: record_principal,
        };
        encode_direct_registration_request_v3_atomic(
            action,
            request,
            requests
                .get_mut(index)
                .ok_or(DirectHotChainFixtureErrorV5::Input)?,
        )
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        *maker_replays
            .get_mut(index)
            .ok_or(DirectHotChainFixtureErrorV5::Input)? = maker_replay;
        *records
            .get_mut(index)
            .ok_or(DirectHotChainFixtureErrorV5::Input)? = record;
        *maker_poststates
            .get_mut(index)
            .ok_or(DirectHotChainFixtureErrorV5::Input)? = created
            .maker_root
            .encode()
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        *record_poststates
            .get_mut(index)
            .ok_or(DirectHotChainFixtureErrorV5::Input)? = created
            .record
            .encode_selected(config, input.geometry.outcome_count())
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        if index == 0 {
            reserved_claims = created.record.reserved_claims();
        } else {
            reserved_collateral = created.record.reserved_collateral();
        }
        root = created.root;
        if index == 0 {
            root_after_sell = Some(root);
        }
    }
    let root_after_sell = root_after_sell.ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    if root.open_maker_root_count() != 2
        || root_after_sell.open_maker_root_count() != 1
        || maker_replays != [capability.seller_maker, capability.buyer_maker]
    {
        return Err(DirectHotChainFixtureErrorV5::Encoding);
    }
    Ok(RegisteredCreationRequestsV4 {
        intents,
        requests,
        maker_replays,
        records,
        root_after_sell,
        root_after_buy: root,
        maker_poststates,
        record_poststates,
        reserved_claims,
        reserved_collateral,
    })
}

fn capability_fixture(
    input: DirectHotChainInputV5,
    manifest: CapabilityManifestFixture,
    market: Pubkey,
) -> Result<CapabilityFixture, DirectHotChainFixtureErrorV5> {
    let CapabilityManifestFixture {
        bytes: manifest,
        selection,
        record_bumps,
    } = manifest;
    let header = CapabilityRootHeaderV1::new(
        core_content(input.release_set)?,
        market.to_bytes(),
        GENERATION,
        selection,
        record_bumps,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut root_bytes = Vec::with_capacity(
        CAPABILITY_ROOT_HEADER_BYTES_V1 + dclutch_trading::successor::DIRECT_ROOT_STATE_BYTES_V1,
    );
    root_bytes.extend_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&DirectRootStateV1::new().encode());
    let (root, root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &input.trading_program);
    let coordinates = DirectCoordinatesV1::new(market.to_bytes(), GENERATION)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let seller_seeds = MakerReplaySeedsV1::new(coordinates, input.makers[0].to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let buyer_seeds = MakerReplaySeedsV1::new(coordinates, input.makers[1].to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let seller_maker =
        Pubkey::find_program_address(&seller_seeds.as_slices(), &input.trading_program).0;
    let buyer_maker =
        Pubkey::find_program_address(&buyer_seeds.as_slices(), &input.trading_program).0;
    Ok(CapabilityFixture {
        root,
        root_bump,
        root_bytes,
        seller_maker,
        buyer_maker,
        manifest,
    })
}

fn capability_manifest(
    input: DirectHotChainInputV5,
    artifacts: &DirectHotArtifactFixtureV5,
    config: &[u8],
) -> Result<CapabilityManifestFixture, DirectHotChainFixtureErrorV5> {
    capability_manifest_selected(input, selected_ordinary_artifacts(artifacts), config)
}

fn capability_manifest_selected(
    input: DirectHotChainInputV5,
    artifacts: SelectedHotArtifactsV5<'_>,
    config: &[u8],
) -> Result<CapabilityManifestFixture, DirectHotChainFixtureErrorV5> {
    let descriptor = CapabilityProgramV4::decode(artifacts.descriptor)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let config_digest = hash(config).to_bytes();
    let amounts = FundingAmountsV1::new(
        CompartmentFundingV1::native_lamports(1)
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let entry = CapabilityEntryV1::new(
        capability_id(descriptor.kind().to_bytes())?,
        capability_id(artifacts.program_set_id)?,
        capability_id(config_digest)?,
        capability_id(descriptor.capacity_profile().to_bytes())?,
        capability_id(descriptor.root_schema().to_bytes())?,
        capability_id(descriptor.derivation_policy().to_bytes())?,
        ActivationPolicy::PrepaidLazy,
        input.clock_slot.saturating_add(100),
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(amounts, None).map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut manifest)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let manifest_digest = hash(&manifest).to_bytes();
    // The activation the chain would have run derives these six bumps once and
    // records them; the fixture stands in for that activation, so it derives
    // exactly the same six from exactly the same seeds.
    let program_set_bumps = record_bumps_v1(
        input.registry_program,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        artifacts.program_set_id,
    );
    let manifest_bumps = record_bumps_v1(
        input.registry_program,
        dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_digest,
    );
    let config_bumps = record_bumps_v1(
        input.registry_program,
        descriptor.config_schema().to_bytes(),
        config_digest,
    );
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        capability_id(manifest_digest)?,
        capability_id(descriptor.kind().to_bytes())?,
        capability_id(artifacts.program_set_id)?,
        capability_id(config_digest)?,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .with_capability_release_record_bumps(program_set_bumps.0, program_set_bumps.1);
    Ok(CapabilityManifestFixture {
        bytes: manifest,
        selection,
        record_bumps: SelectedRecordBumpsV1::new(
            manifest_bumps.0,
            manifest_bumps.1,
            config_bumps.0,
            config_bumps.1,
        ),
    })
}

/// The canonical raw/staging bumps of one finalized record under one Registry.
fn record_bumps_v1(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> (u8, u8) {
    (
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).1,
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).1,
    )
}

fn realm_fixture(
    input: DirectHotChainInputV5,
    collateral: [Pubkey; 3],
    market: Pubkey,
    context: Pubkey,
) -> Result<RealmFixture, DirectHotChainFixtureErrorV5> {
    let token_program = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
    let mint = key(0xa4);
    let realm_bytes = realm_record(collateral)?;
    let realm = finalized(
        input.registry_program,
        REALM_SCHEMA_RELEASE_ID_V1,
        realm_bytes.to_vec(),
    );
    // Direct's escrow replay is Trading-role; the role is a seed component of
    // the replay namespace, so a fixture that derives it without one addresses
    // an account the program will never look at.
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market.to_bytes(),
            input.release_set,
            CallerRoleV1::Trading,
            context.to_bytes(),
        )
        .as_slices(),
        &input.custody_program,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &market.to_bytes(),
            &input.release_set,
        ],
        &input.custody_program,
    )
    .0;
    let replay_bytes = CustodyReplayV1 {
        caller_role: CallerRoleV1::Trading,
        release_set: input.release_set,
        market: market.to_bytes(),
        realm: realm.digest,
        context: context.to_bytes(),
        caller_program: input.trading_program.to_bytes(),
        rent_refund: input.payer.to_bytes(),
        open_vault_count: 0,
        next_revision: CUSTODY_REVISION,
        generation: GENERATION,
        last_request_digest: [0xa7; 32],
        last_poststate_commitment: [0xa8; 32],
    }
    .to_bytes()
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .to_vec();
    let _ = mint;
    Ok(RealmFixture {
        realm,
        mint,
        token_program,
        custody_replay: replay,
        custody_replay_bytes: replay_bytes,
        custody_authority,
    })
}

struct RegisteredBuyCustodyV4 {
    realm: RealmFixture,
    vault: Pubkey,
    route_authorities: [Pubkey; 3],
}

fn registered_buy_custody_v4(
    input: DirectHotChainInputV5,
    rent: &Rent,
    product: &ProductFixture,
    state: &StateFixture,
    requests: &RegisteredCreationRequestsV4,
) -> Result<RegisteredBuyCustodyV4, DirectHotChainFixtureErrorV5> {
    let context = *requests
        .records
        .get(1)
        .ok_or(DirectHotChainFixtureErrorV5::Input)?;
    let token_program = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
    let mint = key(0xa4);
    let realm_bytes = realm_record(product.collateral_accounts)?;
    let realm = finalized(
        input.registry_program,
        REALM_SCHEMA_RELEASE_ID_V1,
        realm_bytes.to_vec(),
    );
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            state.market.to_bytes(),
            input.release_set,
            CallerRoleV1::Trading,
            context.to_bytes(),
        )
        .as_slices(),
        &input.custody_program,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(state.market.to_bytes(), input.release_set).as_slices(),
        &input.custody_program,
    )
    .0;
    let vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            state.market.to_bytes(),
            input.release_set,
            context.to_bytes(),
            CompartmentV1::TradingPrincipal,
        )
        .as_slices(),
        &input.custody_program,
    )
    .0;
    let realm_fixture = RealmFixture {
        realm,
        mint,
        token_program,
        custody_replay: replay,
        custody_replay_bytes: Vec::new(),
        custody_authority,
    };
    let child_requests = registered_buy_child_requests_v4(
        input,
        rent,
        product,
        state,
        requests,
        &realm_fixture,
        vault,
    )?;
    let mut route_authorities = [Pubkey::default(); 3];
    for (index, request) in child_requests.iter().enumerate() {
        let request_digest = hash(request).to_bytes();
        let seeds = CallerAuthoritySeedsV1::new(
            core_content(input.release_set)?,
            state.market.to_bytes(),
            ExecutionRoleV1::Trading,
            context.to_bytes(),
            request_digest,
        )
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        *route_authorities
            .get_mut(index)
            .ok_or(DirectHotChainFixtureErrorV5::Input)? =
            Pubkey::find_program_address(&seeds.as_slices(), &input.trading_program).0;
    }
    Ok(RegisteredBuyCustodyV4 {
        realm: realm_fixture,
        vault,
        route_authorities,
    })
}

#[allow(clippy::too_many_arguments)]
fn registered_buy_child_requests_v4(
    input: DirectHotChainInputV5,
    rent: &Rent,
    product: &ProductFixture,
    state: &StateFixture,
    requests: &RegisteredCreationRequestsV4,
    realm: &RealmFixture,
    vault: Pubkey,
) -> Result<[Vec<u8>; 3], DirectHotChainFixtureErrorV5> {
    let record = *requests
        .records
        .get(1)
        .ok_or(DirectHotChainFixtureErrorV5::Input)?;
    let intent = *requests
        .intents
        .get(1)
        .ok_or(DirectHotChainFixtureErrorV5::Input)?;
    let rent_credit = lifecycle_rent_credit(
        input.rent_program,
        state.market,
        input.release_set,
        input.payer,
    )?
    .0;
    let parent = hash(
        requests
            .requests
            .get(1)
            .ok_or(DirectHotChainFixtureErrorV5::Input)?,
    )
    .to_bytes();
    let common = |operation: OperationV1, transfer_index: u16| CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: input.release_set,
        market: state.market.to_bytes(),
        realm: realm.realm.digest,
        context: record.to_bytes(),
        caller_program: input.trading_program.to_bytes(),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: record.to_bytes(),
            parent_request_digest: parent,
            order_nonce: intent.nonce,
            generation: GENERATION,
            page_index: 0,
            execution_index: intent.outcome,
            transfer_index,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: 0,
        resulting_revision: 0,
        amount: 0,
        rent_lamports: 0,
    };
    // `rent_refund` is the lifecycle RentCredit, and Custody is what says so:
    // `initialize_replay` requires `rent_refund.key == request.rent_refund` AND
    // `rent_refund.key != payer.key`, so the replay's refund account can never
    // be the payer. `ROUTE_ALIASES` already agreed -- the InitializeReplay
    // frame's `RentRefund` coordinate is an alias of
    // `DIRECT_REGISTERED_LIFECYCLE_RENT_CREDIT_ACCOUNT_V4` -- and it was the
    // EFFECT that named a different register.
    let initialize = CustodyRequestV1 {
        payer: input.payer.to_bytes(),
        rent_refund: rent_credit.to_bytes(),
        expected_revision: 0,
        resulting_revision: 1,
        rent_lamports: rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
        ..common(OperationV1::InitializeReplay, 0)
    }
    .to_bytes()
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .to_vec();
    let open = CustodyRequestV1 {
        destination_compartment: CompartmentV1::TradingPrincipal,
        destination: vault.to_bytes(),
        destination_vault_context: record.to_bytes(),
        mint: realm.mint.to_bytes(),
        token_program: realm.token_program.to_bytes(),
        payer: input.payer.to_bytes(),
        rent_refund: rent_credit.to_bytes(),
        expected_revision: 1,
        resulting_revision: 2,
        rent_lamports: rent.minimum_balance(SplAccount::LEN),
        ..common(OperationV1::OpenVault, 1)
    }
    .to_bytes()
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .to_vec();
    let deposit = DelegatedCustodyRequestV2 {
        custody: CustodyRequestV1 {
            operation: OperationV1::Transfer,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::TradingPrincipal,
            semantic: ContextV1 {
                source_owner: input.makers[1].to_bytes(),
                ..common(OperationV1::Transfer, 2).semantic
            },
            source: product.collateral_accounts[0].to_bytes(),
            destination: vault.to_bytes(),
            destination_vault_context: record.to_bytes(),
            mint: realm.mint.to_bytes(),
            token_program: realm.token_program.to_bytes(),
            expected_revision: 2,
            resulting_revision: 3,
            amount: requests.reserved_collateral,
            ..common(OperationV1::Transfer, 2)
        },
        starts_atomic_debit: true,
        terminal: true,
        delegate_before: realm.custody_authority.to_bytes(),
        delegate_after: [0; 32],
        total_debit: requests.reserved_collateral,
        allowance_before: requests.reserved_collateral,
        allowance_after: 0,
    }
    .encode()
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .to_vec();
    Ok([initialize, open, deposit])
}

fn realm_record(
    _collateral: [Pubkey; 3],
) -> Result<[u8; dclutch_market::realm::REALM_BYTES], DirectHotChainFixtureErrorV5> {
    RealmV1::new(RealmV1Input {
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: key(0xa4).to_bytes(),
        // Custody selects the collateral adapter by matching this against
        // `hash(release.to_bytes())` over its own production catalog, and
        // refuses `Realm` when nothing matches. A placeholder digest here was a
        // Realm no live Custody route could ever accept, and it was invisible
        // for as long as nothing reached Custody's body. The legacy exact
        // transfer profile is the one whose `program_id()` is the
        // `token_program` this Realm names.
        collateral_adapter_release_id: hash(&PRODUCTION_ADAPTER_RELEASES[0].to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .map(RealmV1::to_bytes)
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)
}

fn claims_request(
    input: DirectHotChainInputV5,
    product: &ProductFixture,
    state: &StateFixture,
    request: &[u8],
) -> Result<SparseNativeTransferV1, DirectHotChainFixtureErrorV5> {
    SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
        caller_role: ClaimsCallerRole::Trading,
        release_set: input.release_set,
        market: state.market.to_bytes(),
        request_id: hash(request).to_bytes(),
        product_record_digest: product.product.digest,
        semantic_basis_id: product.semantic_basis,
        linked_basis_record_digest: product.basis.digest,
        source_owner: input.makers[0].to_bytes(),
        destination_owner: input.makers[1].to_bytes(),
        expected_market_revision: CLAIMS_MARKET_REVISION,
        expected_source_revision: SELLER_POSITION_REVISION,
        expected_destination_revision: BUYER_POSITION_REVISION,
        generation: GENERATION,
        outcome: 0,
        claim_count: input.geometry.outcome_count(),
        quantity: input.trade.fill,
    })
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)
}

/// The four Custody routes the inline-ordinary Effect declares, in route order.
///
/// The routes are mutually exclusive by construction: the transition derives
/// their enable registers from the combined fee, and exactly one of
/// {`SellerTerminal`}, {`SellerIntermediate`, `FeeContinuation`}, {`FeeSole`}
/// is enabled for any admitted fill. Every one of them nonetheless owns a
/// distinct `CallerAuthority` coordinate in the ninety-one-wide logical vector
/// (34, 48, 62, 76), because the seeds carry that route's own child-request
/// digest, so the fixture has to state four distinct PDAs whichever one the
/// scenario actually invokes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CustodyRouteV3 {
    /// Route 1: the whole seller-net transfer, no fee to follow.
    SellerTerminal,
    /// Route 2: the seller-net transfer that leaves the fee still owed.
    SellerIntermediate,
    /// Route 3: the combined fee that continues route 2's atomic debit.
    FeeContinuation,
    /// Route 4: a fee with no seller-net leg at all.
    FeeSole,
}

impl CustodyRouteV3 {
    /// The route's name, for evidence lines that have to say which leg ran.
    const fn label(self) -> &'static str {
        match self {
            Self::SellerTerminal => "seller-terminal",
            Self::SellerIntermediate => "seller-intermediate",
            Self::FeeContinuation => "fee-continuation",
            Self::FeeSole => "fee-sole",
        }
    }
}

/// The declared route order, which is also the order of the four
/// `CallerAuthority` coordinates 34, 48, 62 and 76.
const CUSTODY_ROUTES_V3: [CustodyRouteV3; 4] = [
    CustodyRouteV3::SellerTerminal,
    CustodyRouteV3::SellerIntermediate,
    CustodyRouteV3::FeeContinuation,
    CustodyRouteV3::FeeSole,
];

/// One Custody route's caller-authority coordinate and the child-request digest
/// that seeds it.
///
/// The digest is carried rather than recomputed because it is the only seed a
/// reader cannot derive from the fixture's other public fields, and it is what
/// makes the address auditable: the four seeds are the release set, the Market,
/// `ExecutionRoleV1::Trading`, the request's own `context`, and this.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyRouteAuthorityV3 {
    /// The address `custody_composition_v3::prepare` will derive for this route.
    pub authority: Pubkey,
    /// SHA-256 of the exact request bytes the Effect projects for this route.
    pub request_digest: [u8; 32],
}

/// The exact registers the transition derives for the scenario this fixture
/// runs, named rather than inlined so the arithmetic is auditable against
/// `crates/dclutch-trading/src/ordinary_v3.rs`.
///
/// `gross = FILL * EXECUTION_PRICE / PRICE_SCALE`, `fee = gross * FEE_BPS /
/// 10_000` (floor), `seller_net = gross - fee`, `buyer_debit = gross + fee`,
/// `combined_fee = fee + fee`. The replay revisions then step once per enabled
/// Custody route: `after_seller = CUSTODY_REVISION + terminal + intermediate`,
/// `after_fee = after_seller + intermediate + fee_sole`.
struct CustodyRegistersV3 {
    seller_net: u64,
    buyer_debit: u64,
    combined_fee: u64,
    after_seller: u64,
    after_fee: u64,
}

fn direct_side_fee(gross: u64) -> Result<u64, DirectHotChainFixtureErrorV5> {
    gross
        .checked_mul(u64::from(FEE_BPS))
        .and_then(|value| value.checked_div(u64::from(DIRECT_FEE_DENOMINATOR_V1)))
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)
}

fn custody_registers(
    trade: DirectTradeScenarioV1,
) -> Result<CustodyRegistersV3, DirectHotChainFixtureErrorV5> {
    let gross = trade
        .fill
        .checked_mul(trade.execution_price)
        .and_then(|value| value.checked_div(PRICE_SCALE))
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    // The market applies the same immutable integer rate independently to the
    // seller and buyer sides. Keep the two floors explicit even though this
    // symmetric fixture has the same gross on each side.
    let seller_fee = direct_side_fee(gross)?;
    let buyer_fee = direct_side_fee(gross)?;
    let seller_net = gross
        .checked_sub(seller_fee)
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    let buyer_debit = gross
        .checked_add(buyer_fee)
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    let combined_fee = seller_fee
        .checked_add(buyer_fee)
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    // `select_zero` leaves the enable register at its loaded constant unless the
    // tested register is zero. Reproduced here as the two booleans it computes.
    let fee_nonzero = combined_fee != 0;
    let seller_terminal = !fee_nonzero;
    let intermediate = fee_nonzero && seller_net != 0;
    let fee_sole = seller_net == 0 && fee_nonzero;
    // The fee continuation has its own enable register now, and the transition
    // pins it to zero: the fee leg settles in a second transaction
    // (`docs/design/FEE_SECOND_TRANSACTION_V1.md`). `fee_sole` is derived and
    // then REQUIRED zero by the transition, because no rate inside the
    // `DIRECT_MAX_FEE_BASIS_POINTS_V1` band can enable it -- this fixture keeps
    // deriving it so a scenario that tried would be visible here too.
    let fee_continuation = false;
    let after_seller = CUSTODY_REVISION
        .checked_add(u64::from(seller_terminal))
        .and_then(|value| value.checked_add(u64::from(intermediate)))
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    let after_fee = after_seller
        .checked_add(u64::from(fee_continuation))
        .and_then(|value| value.checked_add(u64::from(fee_sole)))
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    Ok(CustodyRegistersV3 {
        seller_net,
        buyer_debit,
        combined_fee,
        after_seller,
        after_fee,
    })
}

/// The seller's collateral balance before the trade.
const SELLER_COLLATERAL_BALANCE: u64 = 30;
/// The fee destination's collateral balance before the trade.
const FEE_COLLATERAL_BALANCE: u64 = 40;

/// The buyer's staged collateral balance and the allowance staged on it.
///
/// The allowance is the BUYER DEBIT, `gross + fee`, and not the gross. Under
/// the zero-fee scenario those are the same number -- which is exactly why this
/// fixture was able to stage the gross here for as long as it did -- but the
/// projected `SellerIntermediate` request states `allowance_before = gross +
/// fee` and Custody compares it against the token account's own
/// `delegated_amount`, so under any fee-bearing scenario staging the gross
/// refuses in Custody and reads like a route defect rather than a fixture one.
fn source_collateral(
    trade: DirectTradeScenarioV1,
) -> Result<(u64, u64), DirectHotChainFixtureErrorV5> {
    let registers = custody_registers(trade)?;
    // The three admissibility conditions the on-chain transition would refuse
    // on, checked here so a scenario that cannot trade fails at construction
    // with a name rather than deep inside a projection.
    if trade.source_balance < registers.buyer_debit
        || trade.fill > trade.claim_supply
        || trade.execution_price > PRICE_SCALE
    {
        return Err(DirectHotChainFixtureErrorV5::Input);
    }
    Ok((trade.source_balance, registers.buyer_debit))
}

/// The mint supply, which is the three staged collateral balances.
fn mint_supply(trade: DirectTradeScenarioV1) -> u64 {
    trade
        .source_balance
        .saturating_add(SELLER_COLLATERAL_BALANCE)
        .saturating_add(FEE_COLLATERAL_BALANCE)
}

/// Reproduce one Custody route's projected child request exactly.
///
/// This is the request the Effect program writes into the projected request
/// bank, not a description of it: every field is either a template constant the
/// Effect never overwrites (`operation`, both compartments, `candidate`,
/// `page_index`, the vault contexts, `payer`, `rent_refund`, `rent_lamports`)
/// or the register `push_custody_request` writes at that offset. The Hot
/// executor hashes exactly these bytes to seed the route's caller authority, so
/// any drift here shows up as a `Release` refusal at
/// `require_custody_frame_shape_v3` and nowhere earlier.
///
/// **The scalar registers are patched after encoding, deliberately.** A
/// disabled route's projected request is a well-formed byte string and NOT
/// necessarily a valid `DelegatedCustodyRequestV2`: with a zero fee, the two
/// fee routes carry a zero amount and a zero allowance, which
/// `DelegatedCustodyRequestV2::validate` refuses -- correctly, because a route
/// that never executes is never decoded. The Hot executor still hashes those
/// bytes to derive that route's caller-authority coordinate, so the fixture has
/// to be able to state them. The identity fields come from the encoder, whose
/// output is byte-identical to the projection; only the six scalars the
/// transition derives are written at their layout offsets afterwards.
fn custody_request_bytes(
    route: CustodyRouteV3,
    input: DirectHotChainInputV5,
    product: &ProductFixture,
    state: &StateFixture,
    capability: &CapabilityFixture,
    realm: &RealmFixture,
    request: &[u8],
) -> Result<[u8; DELEGATED_CUSTODY_REQUEST_BYTES_V2], DirectHotChainFixtureErrorV5> {
    let registers = custody_registers(input.trade)?;
    let parent = hash(request).to_bytes();
    let seller_side = matches!(
        route,
        CustodyRouteV3::SellerTerminal | CustodyRouteV3::SellerIntermediate
    );
    let (destination_owner, destination) = if seller_side {
        (input.makers[0], product.collateral_accounts[1])
    } else {
        (input.payer, product.collateral_accounts[2])
    };
    let (expected_revision, resulting_revision) = match route {
        CustodyRouteV3::SellerTerminal | CustodyRouteV3::SellerIntermediate => {
            (CUSTODY_REVISION, registers.after_seller)
        }
        CustodyRouteV3::FeeContinuation => (registers.after_seller, registers.after_fee),
        CustodyRouteV3::FeeSole => (CUSTODY_REVISION, registers.after_fee),
    };
    let amount = if seller_side {
        registers.seller_net
    } else {
        registers.combined_fee
    };
    let allowance_before = match route {
        CustodyRouteV3::FeeContinuation => registers.combined_fee,
        _ => registers.buyer_debit,
    };
    let allowance_after = match route {
        CustodyRouteV3::SellerIntermediate => registers.combined_fee,
        _ => 0,
    };
    // Placeholders that satisfy the delegated-allowance invariants for this
    // route's flag combination; every one of them is overwritten below.
    let (template_total, template_before, template_after, template_amount) = match route {
        CustodyRouteV3::SellerIntermediate => (2, 2, 1, 1),
        CustodyRouteV3::FeeContinuation => (2, 1, 0, 1),
        _ => (1, 1, 0, 1),
    };
    let custody = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::External,
        destination_compartment: CompartmentV1::External,
        release_set: input.release_set,
        market: state.market.to_bytes(),
        realm: realm.realm.digest,
        context: capability.buyer_maker.to_bytes(),
        caller_program: input.trading_program.to_bytes(),
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: input.makers[1].to_bytes(),
            destination_owner: destination_owner.to_bytes(),
            order: parent,
            parent_request_digest: parent,
            order_nonce: 0,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: u16::from(route == CustodyRouteV3::FeeContinuation),
        },
        source: product.collateral_accounts[0].to_bytes(),
        destination: destination.to_bytes(),
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: realm.mint.to_bytes(),
        token_program: realm.token_program.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: CUSTODY_REVISION,
        resulting_revision: CUSTODY_REVISION + 1,
        amount: template_amount,
        rent_lamports: 0,
    };
    let mut bytes = DelegatedCustodyRequestV2 {
        custody,
        starts_atomic_debit: route != CustodyRouteV3::FeeContinuation,
        terminal: route != CustodyRouteV3::SellerIntermediate,
        delegate_before: realm.custody_authority.to_bytes(),
        delegate_after: if route == CustodyRouteV3::SellerIntermediate {
            realm.custody_authority.to_bytes()
        } else {
            [0; 32]
        },
        total_debit: template_total,
        allowance_before: template_before,
        allowance_after: template_after,
    }
    .encode()
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let base = DelegatedCustodyRequestLayoutV2::BASE;
    for (offset, value) in [
        (
            base + CustodyRequestLayoutV1::EXPECTED_REVISION,
            expected_revision,
        ),
        (
            base + CustodyRequestLayoutV1::RESULTING_REVISION,
            resulting_revision,
        ),
        (base + CustodyRequestLayoutV1::AMOUNT, amount),
        (
            DelegatedCustodyRequestLayoutV2::TOTAL_DEBIT,
            registers.buyer_debit,
        ),
        (
            DelegatedCustodyRequestLayoutV2::ALLOWANCE_BEFORE,
            allowance_before,
        ),
        (
            DelegatedCustodyRequestLayoutV2::ALLOWANCE_AFTER,
            allowance_after,
        ),
    ] {
        bytes
            .get_mut(offset..offset + 8)
            .ok_or(DirectHotChainFixtureErrorV5::Encoding)?
            .copy_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}

/// The `CallerAuthority` PDA the Hot executor derives for every Custody route.
///
/// `custody_composition_v3::prepare` is the authority for this derivation:
/// the context seed is the request's own `context` field -- the buyer maker
/// root the Effect projects into every Custody request -- and never the family
/// request digest, which enters only as the fifth seed through the child
/// request's hash.
/// The Claims caller authority this trade's child walk signs under.
///
/// **ONE AUTHOR, AND IT NOW RETURNS ITS BUMP.** The derivation used to sit
/// inline in `logical_accounts`, which installed the ADDRESS and threw the bump
/// away -- and the bump is what `direct_hot_top_level_margin_gate.rs` and
/// `direct_hot_fee_bearing_margin_gate.rs` need, because the search that finds
/// it is the one search on this route neither gate subtracts. Both gates said
/// so in prose and both said the packet digest was "the one seed no public
/// fixture field carries", which was never true: every seed below is a value
/// this function already has.
///
/// The cost of leaving it unsubtracted was measured, not theorised: the floor
/// those gates call key-independent moved 4,836 CU between two builds that
/// compile to byte-identical code -- 948 symbols and 941 stack frames, none
/// differing -- because a relink reseeds this search and the floor is a minimum
/// over draws that include it.
fn claims_caller_authority_v5(
    input: DirectHotChainInputV5,
    product: &ProductFixture,
    state: &StateFixture,
    request: &[u8],
) -> Result<(Pubkey, u8), DirectHotChainFixtureErrorV5> {
    let claims = claims_request(input, product, state, request)?;
    let claims_packet = hash(&claims.to_bytes()).to_bytes();
    let claims_seeds = CallerAuthoritySeedsV1::new(
        core_content(input.release_set)?,
        state.market.to_bytes(),
        ExecutionRoleV1::Trading,
        hash(request).to_bytes(),
        claims_packet,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    Ok(Pubkey::find_program_address(
        &claims_seeds.as_slices(),
        &input.trading_program,
    ))
}

fn custody_route_authorities(
    input: DirectHotChainInputV5,
    product: &ProductFixture,
    state: &StateFixture,
    capability: &CapabilityFixture,
    realm: &RealmFixture,
    request: &[u8],
) -> Result<[CustodyRouteAuthorityV3; 4], DirectHotChainFixtureErrorV5> {
    let mut derived = [CustodyRouteAuthorityV3 {
        authority: Pubkey::default(),
        request_digest: [0; 32],
    }; 4];
    for (slot, route) in derived.iter_mut().zip(CUSTODY_ROUTES_V3) {
        let bytes =
            custody_request_bytes(route, input, product, state, capability, realm, request)?;
        let request_digest = hash(&bytes).to_bytes();
        let seeds = CallerAuthoritySeedsV1::new(
            core_content(input.release_set)?,
            state.market.to_bytes(),
            ExecutionRoleV1::Trading,
            capability.buyer_maker.to_bytes(),
            request_digest,
        )
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
        *slot = CustodyRouteAuthorityV3 {
            authority: Pubkey::find_program_address(&seeds.as_slices(), &input.trading_program).0,
            request_digest,
        };
    }
    Ok(derived)
}

/// One projected Custody leg, in the exact wire and frame Custody admits.
///
/// The Direct inline Effect declares four Custody routes and the transition
/// enables a subset of them; the enabled ones are executed as child CPIs inside
/// the top-level Hot transaction. This type states one route's request BYTES --
/// the same bytes [`custody_request_bytes`] hands the caller-authority
/// derivation, so the digest that seeds the authority is the digest of exactly
/// these -- together with the fourteen accounts
/// `CustodyFrameSpecV1::new(OperationV1::Transfer)` declares for it, in
/// coordinate order.
///
/// It exists so a probe can present one leg to Custody on its own, in its own
/// transaction, without asking Trading to project it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCustodyLegV1 {
    /// Route name in `CUSTODY_ROUTES_V3` order, for evidence lines.
    pub label: &'static str,
    /// The exact projected `DelegatedCustodyRequestV2` bytes.
    pub request: Vec<u8>,
    /// SHA-256 of `request`; the caller authority's sixth seed.
    pub request_digest: [u8; 32],
    /// The Trading caller authority this route's request derives.
    pub authority: Pubkey,
    /// The fourteen Transfer-frame coordinates, in order.
    pub frame: Vec<Pubkey>,
}

/// The four declared Custody legs of one Direct trade scenario.
///
/// Every value is derived from `input` by the same constructors
/// [`build_direct_hot_chain_fixture_v5`] uses, so a leg's request bytes and
/// caller authority are the ones that fixture installs and the artifact builder
/// reproduces.
/// The immutable Direct config record's raw account and its staging cursor.
///
/// Both are content-addressed under the Registry from the same config bytes the
/// fixture installs, so this reproduces the pair rather than describing it. The
/// fee-settlement route reads them to learn the Market's `fee_recipient`, and
/// the vacant staging cursor is how this tree spells "immutable".
///
/// Returned as a pair rather than folded into
/// [`direct_hot_custody_legs_v1`]'s frames because they are not Custody
/// coordinates: Custody never reads the Direct config, and the route that does
/// carries them past the fourteen.
pub fn direct_hot_config_record_v1(
    input: DirectHotChainInputV5,
) -> Result<(Pubkey, Pubkey), DirectHotChainFixtureErrorV5> {
    validate_input(input)?;
    let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
        .encode();
    let record = finalized(
        input.registry_program,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        config.to_vec(),
    );
    Ok((record.raw, record.staging))
}

pub fn direct_hot_custody_legs_v1(
    input: DirectHotChainInputV5,
) -> Result<[DirectCustodyLegV1; 4], DirectHotChainFixtureErrorV5> {
    validate_input(input)?;
    let rent = Rent::default();
    let product = product_fixture(input)?;
    let artifacts = build_direct_hot_artifact_fixture_v5(input.deployment_widths, input.geometry)
        .map_err(DirectHotChainFixtureErrorV5::Artifact)?;
    let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
        .encode();
    let manifest = capability_manifest(input, &artifacts, &config)?;
    let state = market_and_claims(input, &product, &manifest, &rent)?;
    let intents = intents(input, state.market, product.collateral_accounts)?;
    let request = direct_request(input.makers, intents)?;
    let capability = capability_fixture(input, manifest, state.market)?;
    let realm = realm_fixture(
        input,
        product.collateral_accounts,
        state.market,
        capability.buyer_maker,
    )?;
    let authorities =
        custody_route_authorities(input, &product, &state, &capability, &realm, &request)?;
    let mut legs = Vec::with_capacity(CUSTODY_ROUTES_V3.len());
    for (slot, route) in CUSTODY_ROUTES_V3.into_iter().enumerate() {
        let bytes = custody_request_bytes(
            route,
            input,
            &product,
            &state,
            &capability,
            &realm,
            &request,
        )?;
        let derived = authorities
            .get(slot)
            .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
        let seller_side = matches!(
            route,
            CustodyRouteV3::SellerTerminal | CustodyRouteV3::SellerIntermediate
        );
        let destination = product
            .collateral_accounts
            .get(if seller_side { 1 } else { 2 })
            .copied()
            .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
        let source = product
            .collateral_accounts
            .first()
            .copied()
            .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
        // The fourteen Transfer coordinates, in `CustodyFrameSpecV1` order.
        // The same fourteen the Direct AccountProfile lays out at logical
        // coordinates 34..48, 48..62, 62..76 and 76..90.
        let frame = vec![
            derived.authority,
            state.market,
            input.activation_cache,
            input.registry_program,
            input.trading_program,
            input.trading_programdata,
            realm.realm.raw,
            realm.realm.staging,
            realm.custody_replay,
            realm.mint,
            source,
            destination,
            realm.custody_authority,
            realm.token_program,
        ];
        legs.push(DirectCustodyLegV1 {
            label: route.label(),
            request: bytes.to_vec(),
            request_digest: derived.request_digest,
            authority: derived.authority,
            frame,
        });
    }
    let mut drain = legs.into_iter();
    let mut next = || -> Result<DirectCustodyLegV1, DirectHotChainFixtureErrorV5> {
        drain.next().ok_or(DirectHotChainFixtureErrorV5::Encoding)
    };
    Ok([next()?, next()?, next()?, next()?])
}

struct FixedHotAccountsV5 {
    accounts: Vec<ChainAccount>,
    capability_seal: Pubkey,
    capability_seal_bytes: Vec<u8>,
}

fn fixed_hot_accounts(
    input: DirectHotChainInputV5,
    rent: &Rent,
    artifacts: &DirectHotArtifactFixtureV5,
    config: &[u8],
    product: &ProductFixture,
    state: &StateFixture,
    capability: &CapabilityFixture,
) -> Result<FixedHotAccountsV5, DirectHotChainFixtureErrorV5> {
    fixed_hot_accounts_selected(
        input,
        rent,
        selected_ordinary_artifacts(artifacts),
        config,
        product,
        state,
        capability,
    )
}

#[allow(clippy::too_many_arguments)]
fn fixed_hot_accounts_selected(
    input: DirectHotChainInputV5,
    rent: &Rent,
    artifacts: SelectedHotArtifactsV5<'_>,
    config: &[u8],
    product: &ProductFixture,
    state: &StateFixture,
    capability: &CapabilityFixture,
) -> Result<FixedHotAccountsV5, DirectHotChainFixtureErrorV5> {
    let descriptor = CapabilityProgramV4::decode(artifacts.descriptor)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut fixed = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|index| {
            ordinary(
                rent,
                key(u8::try_from(index + 1).unwrap_or(0xfe)),
                system_program::ID,
                Vec::new(),
                index == HOT_ROOT_ACCOUNT_V3,
                false,
            )
        })
        .collect::<Vec<_>>();
    set(
        &mut fixed,
        HOT_MARKET_ACCOUNT_V3,
        owned(
            rent,
            state.market,
            input.core_program,
            state.core_bytes.clone(),
            false,
        ),
    )?;
    set(
        &mut fixed,
        HOT_ROOT_ACCOUNT_V3,
        owned(
            rent,
            capability.root,
            input.trading_program,
            capability.root_bytes.clone(),
            true,
        ),
    )?;
    let finalized_records = [
        (
            HOT_MANIFEST_RAW_ACCOUNT_V3,
            HOT_MANIFEST_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                capability.manifest.clone(),
            ),
        ),
        (
            HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
            HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
                artifacts.program_set.to_vec(),
            ),
        ),
        (
            HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
            HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                CAPABILITY_PROGRAM_SCHEMA_ID_V4,
                artifacts.descriptor.to_vec(),
            ),
        ),
        (
            HOT_CONFIG_RAW_ACCOUNT_V3,
            HOT_CONFIG_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
                config.to_vec(),
            ),
        ),
        (
            HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
            HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.account_profile().schema().to_bytes(),
                artifacts.account_profile.to_vec(),
            ),
        ),
        (
            HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
            HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.request_profile().schema().to_bytes(),
                artifacts.request_profile.to_vec(),
            ),
        ),
        (
            HOT_TRANSITION_RAW_ACCOUNT_V3,
            HOT_TRANSITION_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.transition().schema().to_bytes(),
                artifacts.transition.to_vec(),
            ),
        ),
        (
            HOT_EFFECT_RAW_ACCOUNT_V3,
            HOT_EFFECT_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.effect().schema().to_bytes(),
                artifacts.effect.to_vec(),
            ),
        ),
        (
            HOT_LIFECYCLE_RAW_ACCOUNT_V3,
            HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.lifecycle().schema().to_bytes(),
                artifacts.lifecycle_policy.to_vec(),
            ),
        ),
        (
            HOT_STRATEGY_RAW_ACCOUNT_V3,
            HOT_STRATEGY_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.strategy().schema().to_bytes(),
                artifacts.strategy.to_vec(),
            ),
        ),
    ];
    for (raw, staging, record) in &finalized_records {
        set(&mut fixed, *raw, finalized_raw(rent, record, false))?;
        set(&mut fixed, *staging, vacant(record.staging, false))?;
    }
    // Decision 0005: the validated-artifact seal for exactly this descriptor,
    // this action, this Trading interpreter release and this Registry. The
    // fixture writes the bytes the on-chain seal outer must produce; the
    // continuation campaign proves that it does, byte for byte, rather than
    // assuming it.
    let seal_key = CapabilitySealKeyV1::new(
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        artifacts.descriptor_id,
        artifacts.action as u32,
        input.trading_semantic_release,
        input.registry_program.to_bytes(),
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let (capability_seal, capability_seal_bump) =
        Pubkey::find_program_address(&seal_key.seeds().as_slices(), &input.trading_program);
    let seal_rows = [
        (SealedRoleV1::Descriptor, HOT_DESCRIPTOR_RAW_ACCOUNT_V3),
        (SealedRoleV1::LifecyclePolicy, HOT_LIFECYCLE_RAW_ACCOUNT_V3),
        (
            SealedRoleV1::AccountProfile,
            HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        ),
        (
            SealedRoleV1::RequestProfile,
            HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        ),
        (
            SealedRoleV1::TransitionProgram,
            HOT_TRANSITION_RAW_ACCOUNT_V3,
        ),
        (SealedRoleV1::EffectProgram, HOT_EFFECT_RAW_ACCOUNT_V3),
    ]
    .into_iter()
    .map(|(role, coordinate)| {
        let (_, _, record) = finalized_records
            .iter()
            .find(|(raw, _, _)| *raw == coordinate)
            .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
        SealedRecordRowV1::new(
            role,
            u32::try_from(record.bytes.len()).map_err(|_| DirectHotChainFixtureErrorV5::Input)?,
            record.schema,
            record.digest,
            record.raw.to_bytes(),
            record.staging.to_bytes(),
        )
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)
    })
    .collect::<Result<Vec<_>, _>>()?;
    let seal_rows: [SealedRecordRowV1; CAPABILITY_SEAL_ROW_COUNT_V1] = seal_rows
        .try_into()
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut capability_seal_bytes = vec![0_u8; CAPABILITY_SEAL_BYTES_V1];
    SealedDescriptorClosureV1::encode(
        seal_key,
        seal_rows,
        capability_seal_bump,
        &mut capability_seal_bytes,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    set(
        &mut fixed,
        HOT_CAPABILITY_SEAL_ACCOUNT_V3,
        owned(
            rent,
            capability_seal,
            input.trading_program,
            capability_seal_bytes.clone(),
            false,
        ),
    )?;
    set(
        &mut fixed,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        external_data(
            input.activation_cache,
            input.registry_program,
            dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1,
            false,
            false,
        ),
    )?;
    set(
        &mut fixed,
        HOT_CORE_PROGRAM_ACCOUNT_V3,
        program(input.core_program),
    )?;
    set(
        &mut fixed,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        programdata(
            input.core_programdata,
            input.deployment_widths.core_programdata_bytes,
        ),
    )?;
    set(
        &mut fixed,
        HOT_TRADING_PROGRAM_ACCOUNT_V3,
        program(input.trading_program),
    )?;
    set(
        &mut fixed,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
        programdata(
            input.trading_programdata,
            input.deployment_widths.trading_programdata_bytes,
        ),
    )?;
    set(
        &mut fixed,
        HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        program(input.registry_program),
    )?;
    set(
        &mut fixed,
        HOT_RENT_SYSVAR_ACCOUNT_V3,
        external_empty(sysvar::rent::ID, sysvar::ID, false, false),
    )?;
    set(
        &mut fixed,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        external_empty(sysvar::instructions::ID, sysvar::ID, false, false),
    )?;
    for (raw, staging, record) in [
        (
            HOT_PRODUCT_RAW_ACCOUNT_V3,
            HOT_PRODUCT_STAGING_ACCOUNT_V3,
            &product.product,
        ),
        (
            HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
            HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3,
            &product.domain,
        ),
        (
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            HOT_PORTFOLIO_STAGING_ACCOUNT_V3,
            &product.portfolio,
        ),
        (
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
            HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
            &product.basis,
        ),
    ] {
        set(&mut fixed, raw, finalized_raw(rent, record, false))?;
        set(&mut fixed, staging, vacant(record.staging, false))?;
    }
    Ok(FixedHotAccountsV5 {
        accounts: fixed,
        capability_seal,
        capability_seal_bytes,
    })
}

#[allow(clippy::too_many_arguments)]
fn logical_accounts(
    input: DirectHotChainInputV5,
    rent: &Rent,
    artifacts: &DirectHotArtifactFixtureV5,
    config: &[u8],
    product: &ProductFixture,
    state: &StateFixture,
    capability: &CapabilityFixture,
    realm: &RealmFixture,
    request: &[u8],
    custody_routes: &[CustodyRouteAuthorityV3; 4],
) -> Result<Vec<ChainAccount>, DirectHotChainFixtureErrorV5> {
    let custody_route_account =
        |index: usize| -> Result<ChainAccount, DirectHotChainFixtureErrorV5> {
            Ok(external_empty(
                custody_routes
                    .get(index)
                    .ok_or(DirectHotChainFixtureErrorV5::Encoding)?
                    .authority,
                system_program::ID,
                false,
                false,
            ))
        };
    let mut logical = (0..usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3))
        .map(|index| {
            ordinary(
                rent,
                key(u8::try_from(index + 0xc0).unwrap_or(0xfd)),
                system_program::ID,
                Vec::new(),
                false,
                false,
            )
        })
        .collect::<Vec<_>>();
    let fixed =
        fixed_hot_accounts(input, rent, artifacts, config, product, state, capability)?.accounts;
    for (logical_index, fixed_index) in [
        (0, HOT_ROOT_ACCOUNT_V3),
        (1, HOT_CONFIG_RAW_ACCOUNT_V3),
        (2, HOT_PRODUCT_RAW_ACCOUNT_V3),
        (3, HOT_PORTFOLIO_RAW_ACCOUNT_V3),
        (4, HOT_LINKED_BASIS_RAW_ACCOUNT_V3),
    ] {
        set(
            &mut logical,
            logical_index,
            fixed
                .get(fixed_index)
                .ok_or(DirectHotChainFixtureErrorV5::Profile)?
                .clone(),
        )?;
    }
    set(&mut logical, 5, vacant(capability.seller_maker, true))?;
    set(&mut logical, 6, external_payer(input.payer))?;
    set(
        &mut logical,
        7,
        lifecycle_rent_credit_account(rent, input, state.market)?,
    )?;
    set(&mut logical, 8, vacant(capability.buyer_maker, true))?;
    // Coordinate 9 is the authenticated route alias of the sole payer at 6.
    set(&mut logical, 9, external_payer(input.payer))?;
    set(&mut logical, 10, program(input.rent_program))?;
    set(&mut logical, 11, program(system_program::ID))?;

    let (claims_authority, _claims_authority_bump) =
        claims_caller_authority_v5(input, product, state, request)?;
    set(
        &mut logical,
        12,
        external_empty(claims_authority, system_program::ID, false, false),
    )?;
    set(
        &mut logical,
        13,
        owned(
            rent,
            state.claims_market,
            input.claims_program,
            state.claims_bytes.clone(),
            true,
        ),
    )?;
    set(&mut logical, 14, finalized_raw(rent, &product.basis, false))?;
    set(&mut logical, 15, vacant(product.basis.staging, false))?;
    set(
        &mut logical,
        16,
        finalized_raw(rent, &product.product, false),
    )?;
    set(&mut logical, 17, vacant(product.product.staging, false))?;
    set(
        &mut logical,
        18,
        finalized_raw(rent, &product.domain, false),
    )?;
    set(&mut logical, 19, vacant(product.domain.staging, false))?;
    set(
        &mut logical,
        20,
        finalized_raw(rent, &product.portfolio, false),
    )?;
    set(&mut logical, 21, vacant(product.portfolio.staging, false))?;
    set(
        &mut logical,
        22,
        external_empty(sysvar::rent::ID, sysvar::ID, false, false),
    )?;
    set(
        &mut logical,
        23,
        owned(
            rent,
            state.market,
            input.core_program,
            state.core_bytes.clone(),
            false,
        ),
    )?;
    set(
        &mut logical,
        24,
        external_data(
            input.activation_cache,
            input.registry_program,
            dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1,
            false,
            false,
        ),
    )?;
    set(&mut logical, 25, program(input.registry_program))?;
    set(&mut logical, 26, program(input.trading_program))?;
    set(
        &mut logical,
        27,
        programdata(
            input.trading_programdata,
            input.deployment_widths.trading_programdata_bytes,
        ),
    )?;
    set(&mut logical, 28, program(input.claims_program))?;
    set(
        &mut logical,
        29,
        programdata(
            input.claims_programdata,
            input.deployment_widths.claims_programdata_bytes,
        ),
    )?;
    set(&mut logical, 30, program(input.core_program))?;
    set(
        &mut logical,
        31,
        programdata(
            input.core_programdata,
            input.deployment_widths.core_programdata_bytes,
        ),
    )?;
    set(
        &mut logical,
        32,
        owned(
            rent,
            state.positions[0].0,
            input.claims_program,
            state.positions[0].1.clone(),
            true,
        ),
    )?;
    set(
        &mut logical,
        33,
        owned(
            rent,
            state.positions[1].0,
            input.claims_program,
            state.positions[1].1.clone(),
            true,
        ),
    )?;

    set(&mut logical, 34, custody_route_account(0)?)?;
    for (alias, representative) in [(35, 23), (36, 24), (37, 25), (38, 26), (39, 27)] {
        let value = logical
            .get(representative)
            .ok_or(DirectHotChainFixtureErrorV5::Profile)?
            .clone();
        set(&mut logical, alias, value)?;
    }
    set(&mut logical, 40, finalized_raw(rent, &realm.realm, false))?;
    set(&mut logical, 41, vacant(realm.realm.staging, false))?;
    set(
        &mut logical,
        42,
        owned(
            rent,
            realm.custody_replay,
            input.custody_program,
            realm.custody_replay_bytes.clone(),
            true,
        ),
    )?;
    set(
        &mut logical,
        43,
        owned(
            rent,
            realm.mint,
            realm.token_program,
            mint_bytes(mint_supply(input.trade)),
            false,
        ),
    )?;
    let (source_balance, allowance) = source_collateral(input.trade)?;
    set(
        &mut logical,
        44,
        owned(
            rent,
            product.collateral_accounts[0],
            realm.token_program,
            token_bytes(
                realm.mint,
                input.makers[1],
                source_balance,
                Some(realm.custody_authority),
                allowance,
            )?,
            true,
        ),
    )?;
    set(
        &mut logical,
        45,
        owned(
            rent,
            product.collateral_accounts[1],
            realm.token_program,
            token_bytes(
                realm.mint,
                input.makers[0],
                SELLER_COLLATERAL_BALANCE,
                None,
                0,
            )?,
            true,
        ),
    )?;
    set(
        &mut logical,
        46,
        external_empty(realm.custody_authority, system_program::ID, false, false),
    )?;
    set(&mut logical, 47, program(realm.token_program))?;
    set(
        &mut logical,
        73,
        owned(
            rent,
            product.collateral_accounts[2],
            realm.token_program,
            token_bytes(realm.mint, input.payer, FEE_COLLATERAL_BALANCE, None, 0)?,
            true,
        ),
    )?;
    // Each child route's `CallerAuthority` is a distinct Trading PDA: its seeds
    // carry that route's own child-request digest, so two routes never share
    // one. Coordinates 48, 62 and 76 were literal `key(0xb0/0xb1/0xb2)` --
    // distinct, which is all `validate_accounts` asks, and PDAs of nothing.
    // Whichever route the fee makes live refuses at
    // `require_custody_frame_shape_v3` against a placeholder, so the fixture
    // states all four the way the runtime derives them.
    set(&mut logical, 48, custody_route_account(1)?)?;
    for (alias, representative) in [
        (49, 23),
        (50, 24),
        (51, 25),
        (52, 26),
        (53, 27),
        (54, 40),
        (55, 41),
        (56, 42),
        (57, 43),
        (58, 44),
        (59, 45),
        (60, 46),
        (61, 47),
        (63, 23),
        (64, 24),
        (65, 25),
        (66, 26),
        (67, 27),
        (68, 40),
        (69, 41),
        (70, 42),
        (71, 43),
        (72, 44),
        (74, 46),
        (75, 47),
        (77, 23),
        (78, 24),
        (79, 25),
        (80, 26),
        (81, 27),
        (82, 40),
        (83, 41),
        (84, 42),
        (85, 43),
        (86, 44),
        (87, 73),
        (88, 46),
        (89, 47),
    ] {
        let value = logical
            .get(representative)
            .ok_or(DirectHotChainFixtureErrorV5::Profile)?
            .clone();
        set(&mut logical, alias, value)?;
    }
    set(&mut logical, 62, custody_route_account(2)?)?;
    set(&mut logical, 76, custody_route_account(3)?)?;
    // The four Custody routes are invoked through the Custody program the
    // activated release set selects, and the Hot executor resolves it by
    // scanning the effect accounts for that key. A Custody `Transfer` frame
    // never names its own callee, so the topology carries it here, past every
    // route range, as a readonly executable account of its own.
    set(
        &mut logical,
        usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3),
        program(input.custody_program),
    )?;
    Ok(logical)
}

#[allow(clippy::too_many_arguments)]
fn registered_creation_logical_accounts_v4(
    input: DirectHotChainInputV5,
    rent: &Rent,
    artifacts: SelectedHotArtifactsV5<'_>,
    config: &[u8],
    product: &ProductFixture,
    state: &StateFixture,
    capability: &CapabilityFixture,
    requests: &RegisteredCreationRequestsV4,
    custody: Option<&RegisteredBuyCustodyV4>,
) -> Result<Vec<ChainAccount>, DirectHotChainFixtureErrorV5> {
    let (logical_count, participant) = match artifacts.action {
        DirectExecutionActionV3::RegisterSell => {
            (usize::from(DIRECT_REGISTER_SELL_FIXED_ACCOUNTS_V4), 0)
        }
        DirectExecutionActionV3::RegisterBuy => {
            (usize::from(DIRECT_REGISTER_BUY_FIXED_ACCOUNTS_V4), 1)
        }
        _ => return Err(DirectHotChainFixtureErrorV5::Input),
    };
    let mut logical = (0..logical_count)
        .map(|index| {
            ordinary(
                rent,
                key(u8::try_from(index + 0x70).unwrap_or(0xfc)),
                system_program::ID,
                Vec::new(),
                false,
                false,
            )
        })
        .collect::<Vec<_>>();
    let fixed =
        fixed_hot_accounts_selected(input, rent, artifacts, config, product, state, capability);
    let fixed = match fixed {
        Ok(value) => value.accounts,
        Err(error) => return Err(error),
    };
    for (logical_index, fixed_index) in [
        (0, HOT_ROOT_ACCOUNT_V3),
        (1, HOT_CONFIG_RAW_ACCOUNT_V3),
        (2, HOT_PRODUCT_RAW_ACCOUNT_V3),
        (3, HOT_PORTFOLIO_RAW_ACCOUNT_V3),
        (4, HOT_LINKED_BASIS_RAW_ACCOUNT_V3),
    ] {
        set(
            &mut logical,
            logical_index,
            fixed
                .get(fixed_index)
                .ok_or(DirectHotChainFixtureErrorV5::Profile)?
                .clone(),
        )?;
    }
    set(
        &mut logical,
        5,
        vacant(
            *requests
                .maker_replays
                .get(participant)
                .ok_or(DirectHotChainFixtureErrorV5::Input)?,
            true,
        ),
    )?;
    set(&mut logical, 6, external_payer(input.payer))?;
    set(
        &mut logical,
        7,
        lifecycle_rent_credit_account(rent, input, state.market)?,
    )?;
    set(
        &mut logical,
        8,
        vacant(
            *requests
                .records
                .get(participant)
                .ok_or(DirectHotChainFixtureErrorV5::Input)?,
            true,
        ),
    )?;
    set(&mut logical, 9, external_payer(input.payer))?;
    set(&mut logical, 10, program(input.rent_program))?;
    set(&mut logical, 11, program(system_program::ID))?;

    if artifacts.action == DirectExecutionActionV3::RegisterSell {
        set(
            &mut logical,
            usize::from(DIRECT_REGISTER_SELL_COLLATERAL_ACCOUNT_V4),
            owned(
                rent,
                product.collateral_accounts[1],
                Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
                token_bytes(
                    key(0xa4),
                    input.makers[0],
                    SELLER_COLLATERAL_BALANCE,
                    None,
                    0,
                )?,
                true,
            ),
        )?;
        return Ok(logical);
    }

    let custody = custody.ok_or(DirectHotChainFixtureErrorV5::Input)?;
    for (start, operation, authority) in [
        (
            DIRECT_REGISTER_BUY_INITIALIZE_ACCOUNT_START_V4,
            OperationV1::InitializeReplay,
            custody.route_authorities[0],
        ),
        (
            DIRECT_REGISTER_BUY_OPEN_ACCOUNT_START_V4,
            OperationV1::OpenVault,
            custody.route_authorities[1],
        ),
        (
            DIRECT_REGISTER_BUY_DEPOSIT_ACCOUNT_START_V4,
            OperationV1::Transfer,
            custody.route_authorities[2],
        ),
    ] {
        let frame = registered_buy_custody_frame_accounts_v4(
            input, rent, product, state, requests, custody, operation, authority,
        )?;
        for (offset, account) in frame.into_iter().enumerate() {
            set(
                &mut logical,
                usize::from(start)
                    .checked_add(offset)
                    .ok_or(DirectHotChainFixtureErrorV5::Profile)?,
                account,
            )?;
        }
    }
    set(
        &mut logical,
        usize::from(DIRECT_REGISTER_BUY_CUSTODY_PROGRAM_ACCOUNT_V4),
        program(input.custody_program),
    )?;
    Ok(logical)
}

#[allow(clippy::too_many_arguments)]
fn registered_buy_custody_frame_accounts_v4(
    input: DirectHotChainInputV5,
    rent: &Rent,
    product: &ProductFixture,
    state: &StateFixture,
    requests: &RegisteredCreationRequestsV4,
    custody: &RegisteredBuyCustodyV4,
    operation: OperationV1,
    authority: Pubkey,
) -> Result<Vec<ChainAccount>, DirectHotChainFixtureErrorV5> {
    let spec = CustodyFrameSpecV1::new(operation);
    let mut output = Vec::with_capacity(usize::from(spec.account_count()));
    for index in 0..spec.account_count() {
        let role = spec
            .account(index)
            .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?
            .role();
        let account = match role {
            CustodyFrameRoleV1::CallerAuthority => {
                external_empty(authority, system_program::ID, false, false)
            }
            CustodyFrameRoleV1::CoreMarket => owned(
                rent,
                state.market,
                input.core_program,
                state.core_bytes.clone(),
                false,
            ),
            CustodyFrameRoleV1::ActivationCache => external_data(
                input.activation_cache,
                input.registry_program,
                dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1,
                false,
                false,
            ),
            CustodyFrameRoleV1::RegistryProgram => program(input.registry_program),
            CustodyFrameRoleV1::CallerProgram => program(input.trading_program),
            CustodyFrameRoleV1::CallerProgramData => programdata(
                input.trading_programdata,
                input.deployment_widths.trading_programdata_bytes,
            ),
            CustodyFrameRoleV1::RealmRecord => finalized_raw(rent, &custody.realm.realm, false),
            CustodyFrameRoleV1::RealmStaging => vacant(custody.realm.realm.staging, false),
            CustodyFrameRoleV1::Replay => vacant(custody.realm.custody_replay, true),
            CustodyFrameRoleV1::Payer => external_payer(input.payer),
            CustodyFrameRoleV1::SystemProgram => program(system_program::ID),
            CustodyFrameRoleV1::RentSysvar => {
                external_data(sysvar::rent::ID, sysvar::ID, 17, false, false)
            }
            CustodyFrameRoleV1::Mint => owned(
                rent,
                custody.realm.mint,
                custody.realm.token_program,
                mint_bytes(mint_supply(input.trade)),
                false,
            ),
            CustodyFrameRoleV1::Vault | CustodyFrameRoleV1::TransferDestination => {
                vacant(custody.vault, true)
            }
            CustodyFrameRoleV1::CustodyAuthority => external_empty(
                custody.realm.custody_authority,
                system_program::ID,
                false,
                false,
            ),
            CustodyFrameRoleV1::TokenProgram => program(custody.realm.token_program),
            CustodyFrameRoleV1::TransferSource => owned(
                rent,
                product.collateral_accounts[0],
                custody.realm.token_program,
                token_bytes(
                    custody.realm.mint,
                    input.makers[1],
                    input.trade.source_balance,
                    Some(custody.realm.custody_authority),
                    requests.reserved_collateral,
                )?,
                true,
            ),
            CustodyFrameRoleV1::RentRefund => {
                lifecycle_rent_credit_account(rent, input, state.market)?
            }
        };
        output.push(account);
    }
    Ok(output)
}

fn pack_runtime(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    logical: &mut [ChainAccount],
) -> Result<Vec<ChainAccount>, DirectHotChainFixtureErrorV5> {
    if logical.len()
        != profile
            .logical_account_count(tail_count)
            .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?
    {
        return Err(DirectHotChainFixtureErrorV5::Profile);
    }
    let count = profile
        .physical_account_count_with_dynamic_spans(tail_count, &[])
        .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
    let mut packed: Vec<Option<ChainAccount>> = vec![None; count];
    for (coordinate, value) in logical.iter().enumerate() {
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(tail_count, &[], coordinate)
            .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
        match packed
            .get_mut(ordinal)
            .ok_or(DirectHotChainFixtureErrorV5::Profile)?
        {
            Some(existing) if existing.key != value.key || existing.account != value.account => {
                return Err(DirectHotChainFixtureErrorV5::Profile);
            }
            Some(existing) => existing.snapshot |= value.snapshot,
            slot @ None => *slot = Some(value.clone()),
        }
    }
    packed
        .into_iter()
        .enumerate()
        .map(|(ordinal, value)| {
            let mut value = value.ok_or(DirectHotChainFixtureErrorV5::Profile)?;
            let geometry = profile
                .physical_account_geometry_with_dynamic_spans(tail_count, &[], ordinal)
                .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
            value.meta.is_writable = geometry.privileges().writable();
            value.meta.is_signer = value.key
                == logical
                    .get(6)
                    .ok_or(DirectHotChainFixtureErrorV5::Profile)?
                    .key
                || value.key
                    == logical
                        .get(9)
                        .ok_or(DirectHotChainFixtureErrorV5::Profile)?
                        .key;
            Ok(value)
        })
        .collect()
}

fn set(
    values: &mut [ChainAccount],
    index: usize,
    value: ChainAccount,
) -> Result<(), DirectHotChainFixtureErrorV5> {
    *values
        .get_mut(index)
        .ok_or(DirectHotChainFixtureErrorV5::Profile)? = value;
    Ok(())
}

fn owned(rent: &Rent, key: Pubkey, owner: Pubkey, data: Vec<u8>, writable: bool) -> ChainAccount {
    ordinary(rent, key, owner, data, writable, false)
}

fn ordinary(
    rent: &Rent,
    key: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
    writable: bool,
    signer: bool,
) -> ChainAccount {
    let lamports = if data.is_empty() {
        0
    } else {
        rent.minimum_balance(data.len())
    };
    ChainAccount {
        key,
        account: Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
        meta: AccountMeta {
            pubkey: key,
            is_signer: signer,
            is_writable: writable,
        },
        snapshot: writable,
    }
}

fn vacant(key: Pubkey, writable: bool) -> ChainAccount {
    external_empty(key, system_program::ID, false, writable)
}

fn external_empty(key: Pubkey, owner: Pubkey, signer: bool, writable: bool) -> ChainAccount {
    ChainAccount {
        key,
        account: Account {
            lamports: 0,
            data: Vec::new(),
            owner,
            executable: false,
            rent_epoch: 0,
        },
        meta: AccountMeta {
            pubkey: key,
            is_signer: signer,
            is_writable: writable,
        },
        snapshot: writable,
    }
}

fn external_payer(key: Pubkey) -> ChainAccount {
    let mut account = external_empty(key, system_program::ID, true, true);
    account.snapshot = false;
    account
}

fn external_data(
    key: Pubkey,
    owner: Pubkey,
    _bytes: usize,
    signer: bool,
    writable: bool,
) -> ChainAccount {
    external_empty(key, owner, signer, writable)
}

fn program(key: Pubkey) -> ChainAccount {
    ChainAccount {
        key,
        account: Account {
            lamports: 1,
            data: Vec::new(),
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
        },
        meta: AccountMeta::new_readonly(key, false),
        snapshot: false,
    }
}

fn programdata(key: Pubkey, _bytes: u32) -> ChainAccount {
    external_empty(key, bpf_loader_upgradeable::ID, false, false)
}

fn finalized_raw(rent: &Rent, record: &Finalized, writable: bool) -> ChainAccount {
    owned(
        rent,
        record.raw,
        record.owner,
        record.bytes.clone(),
        writable,
    )
}

/// The sole Market-lifecycle RentCredit.
///
/// `LifecycleRentCreditV2` is keyed by Market and generation alone, so one
/// credit serves the whole Market lifecycle and both replay-root creations. The
/// adapter re-derives it from the credit account's own owner, which is why the
/// Rent program is a coordinate of the Direct profile.
fn lifecycle_rent_credit(
    rent_program: Pubkey,
    market: Pubkey,
    release_set: [u8; 32],
    refund_wallet: Pubkey,
) -> Result<(Pubkey, LifecycleRentCreditV2), DirectHotChainFixtureErrorV5> {
    let (key, bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &rent_program,
    );
    let credit = LifecycleRentCreditV2::new(
        RefundAuthority::new(refund_wallet.to_bytes())
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        LifecycleAccountIdV2::new(market.to_bytes())
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        LifecycleAccountIdV2::new(release_set)
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        GENERATION,
        bump,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    Ok((key, credit))
}

fn lifecycle_rent_credit_account(
    rent: &Rent,
    input: DirectHotChainInputV5,
    market: Pubkey,
) -> Result<ChainAccount, DirectHotChainFixtureErrorV5> {
    let (key, credit) =
        lifecycle_rent_credit(input.rent_program, market, input.release_set, input.payer)?;
    // `owned` funds an account to the current rent minimum for its own data
    // width, which is exactly the rent exemption the adapter requires of the
    // credit at its 128 bytes.
    Ok(owned(
        rent,
        key,
        input.rent_program,
        credit.to_bytes().to_vec(),
        true,
    ))
}

fn mint_bytes(supply: u64) -> Vec<u8> {
    let mut output = vec![0_u8; SplMint::LEN];
    let value = SplMint {
        mint_authority: COption::None,
        supply,
        decimals: 0,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    if SplMint::pack(value, &mut output).is_err() {
        output.fill(0);
    }
    output
}

fn token_bytes(
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    delegate: Option<Pubkey>,
    delegated_amount: u64,
) -> Result<Vec<u8>, DirectHotChainFixtureErrorV5> {
    let mut output = vec![0_u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount,
            delegate: delegate.map_or(COption::None, COption::Some),
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount,
            close_authority: COption::None,
        },
        &mut output,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    Ok(output)
}

fn capability_id(value: [u8; 32]) -> Result<CapabilityContentId, DirectHotChainFixtureErrorV5> {
    CapabilityContentId::new(value).map_err(|_| DirectHotChainFixtureErrorV5::Input)
}

fn core_content(value: [u8; 32]) -> Result<CoreContentId, DirectHotChainFixtureErrorV5> {
    CoreContentId::new(value).map_err(|_| DirectHotChainFixtureErrorV5::Input)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;

    use super::*;

    fn input() -> DirectHotChainInputV5 {
        DirectHotChainInputV5 {
            registry_program: key(1),
            trading_program: key(2),
            core_program: key(3),
            claims_program: key(4),
            custody_program: key(5),
            rent_program: key(14),
            release_set: [6; 32],
            activation_cache: key(7),
            trading_programdata: key(8),
            core_programdata: key(9),
            claims_programdata: key(10),
            deployment_widths: DirectHotDeploymentWidthsV5::new(1_141_117, 971_053, 934_037)
                .expect("widths"),
            payer: key(11),
            makers: [key(12), key(13)],
            clock_slot: 50,
            geometry: DirectOrdinaryGeometryV3::CANONICAL,
            trading_semantic_release: [0x33; 32],
            trade: DirectTradeScenarioV1::ZERO_FEE,
        }
    }

    #[test]
    fn development_market_fee_is_fifty_basis_points_per_side_with_flooring() {
        assert_eq!(FEE_BPS, 50);
        assert_eq!(direct_side_fee(5).expect("small-gross fee"), 0);
        let seller_fee = direct_side_fee(1_000).expect("seller fee");
        let buyer_fee = direct_side_fee(1_000).expect("buyer fee");
        assert_eq!(seller_fee, 5);
        assert_eq!(buyer_fee, 5);
        assert_eq!(seller_fee.checked_add(buyer_fee), Some(10));
    }

    /// The two scenarios differ in exactly one thing: WHICH Custody route the
    /// transition enables, and whether it is terminal.
    ///
    /// Asserted here, off-chain and in a hundred microseconds, because the
    /// forty-second on-chain sweep that measures the fee leg is worth nothing
    /// if the scenario it runs turns out to have floored its fee too. The
    /// enable arithmetic is `custody_registers`' own reproduction of
    /// `select_zero`, so this test is checking the fixture's model against the
    /// scenario constants and NOT checking the chain -- the chain's answer is
    /// the invocation count in `direct_hot_fee_bearing_margin_gate.rs`.
    #[test]
    fn only_the_fee_bearing_scenario_leaves_the_seller_route_non_terminal() {
        let zero = custody_registers(DirectTradeScenarioV1::ZERO_FEE).expect("zero-fee registers");
        assert_eq!(
            zero.combined_fee, 0,
            "the historical fixture floors its fee"
        );
        assert_eq!(zero.seller_net, 5);
        assert_eq!(zero.buyer_debit, 5);
        // One route: `after_seller` steps once for `seller_terminal` and
        // `after_fee` does not step again.
        assert_eq!(zero.after_seller, CUSTODY_REVISION + 1);
        assert_eq!(zero.after_fee, CUSTODY_REVISION + 1);

        let fee = custody_registers(DirectTradeScenarioV1::FEE_BEARING).expect("fee registers");
        assert_eq!(fee.combined_fee, 2, "gross 200 at 50 bps per side");
        // The protocol bound the scenario has to respect to exist at all.
        assert!(DirectTradeScenarioV1::FEE_BEARING.execution_price() <= PRICE_SCALE);
        assert!(
            DirectTradeScenarioV1::FEE_BEARING.fill()
                <= DirectTradeScenarioV1::FEE_BEARING.claim_supply()
        );
        assert_eq!(fee.seller_net, 199);
        assert_eq!(fee.buyer_debit, 201);
        // ONE route here too, and that is the change. `SellerIntermediate`
        // steps `after_seller` once; the fee continuation no longer steps
        // `after_fee` on top of it, because the fee leg settles in a second
        // transaction (`docs/design/FEE_SECOND_TRANSACTION_V1.md`). What
        // distinguishes the two scenarios is not the route COUNT any more but
        // which slot runs and whether the delegation survives it: the zero-fee
        // scenario takes terminal slot 0 and closes it, the fee-bearing one
        // takes non-terminal slot 1 and leaves `combined_fee` standing for the
        // second transaction to spend.
        assert_eq!(fee.after_seller, CUSTODY_REVISION + 1);
        assert_eq!(fee.after_fee, CUSTODY_REVISION + 1);

        // The buyer's staged allowance is the DEBIT, and under the fee-bearing
        // scenario that is not the gross.
        let (balance, allowance) =
            source_collateral(DirectTradeScenarioV1::FEE_BEARING).expect("fee collateral");
        assert_eq!(allowance, 201);
        assert!(balance >= allowance);
        assert_eq!(
            source_collateral(DirectTradeScenarioV1::ZERO_FEE).expect("zero-fee collateral"),
            (100, 5),
            "the zero-fee scenario must stage exactly what it always staged",
        );
        assert_eq!(mint_supply(DirectTradeScenarioV1::ZERO_FEE), 170);
        assert_eq!(DirectTradeScenarioV1::ZERO_FEE.claim_supply(), 100);
    }

    #[test]
    fn complete_fixture_packs_one_profile13_authority() {
        let input = input();
        let rent = Rent::default();
        let artifacts =
            build_direct_hot_artifact_fixture_v5(input.deployment_widths, input.geometry)
                .expect("artifacts");
        let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
            .expect("config")
            .encode();
        let product = product_fixture(input).expect("product");
        let manifest = capability_manifest(input, &artifacts, &config).expect("manifest");
        let state = market_and_claims(input, &product, &manifest, &rent).expect("state");
        let intents = intents(input, state.market, product.collateral_accounts).expect("intents");
        let request = direct_request(input.makers, intents).expect("request");
        let capability = capability_fixture(input, manifest, state.market).expect("capability");
        let realm = realm_fixture(
            input,
            product.collateral_accounts,
            state.market,
            capability.buyer_maker,
        )
        .expect("realm");
        let custody_routes =
            custody_route_authorities(input, &product, &state, &capability, &realm, &request)
                .expect("custody routes");
        logical_accounts(
            input,
            &rent,
            &artifacts,
            &config,
            &product,
            &state,
            &capability,
            &realm,
            &request,
            &custody_routes,
        )
        .expect("logical");
        let fixture = build_direct_hot_chain_fixture_v5(input).expect("chain fixture");
        assert_eq!(fixture.hot_instruction.program_id, key(2));
        assert_eq!(
            fixture.hot_instruction.data.len(),
            HOT_EXECUTION_ENVELOPE_BYTES_V3 + DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3
        );
        let profile =
            AccountProfileV2::decode(&artifacts.bundle.account_profile).expect("AccountProfile");
        let physical = profile
            .physical_account_count_with_dynamic_spans(input.geometry.outcome_count(), &[])
            .expect("physical account count");
        assert_eq!(
            fixture.hot_instruction.accounts.len(),
            HOT_FIXED_ACCOUNT_COUNT_V3 + physical - 5
        );
        assert!(
            fixture
                .accounts
                .iter()
                .any(|value| value.key == fixture.root)
        );
        assert!(
            fixture
                .rollback_snapshot_keys
                .iter()
                .all(|key| fixture.accounts.iter().any(|value| value.key == *key))
        );
        assert!(fixture.externally_installed_keys.contains(&input.payer));
        assert!(!fixture.rollback_snapshot_keys.contains(&input.payer));
    }

    #[test]
    fn registered_creation_fixture_is_one_ordered_sell_buy_chain() {
        let input = input();
        let fixture =
            build_direct_registered_creation_chain_fixture_v4(input).expect("registered chain");
        assert_registered_creation_hot_hints_v4(&fixture);
        assert_eq!(
            fixture.sell_hot_instruction.program_id,
            input.trading_program
        );
        assert_eq!(
            fixture.buy_hot_instruction.program_id,
            input.trading_program
        );
        assert_eq!(
            fixture.sell_hot_instruction.data.len(),
            HOT_EXECUTION_ENVELOPE_BYTES_V3 + DIRECT_REGISTRATION_REQUEST_BYTES_V3
        );
        assert_eq!(
            fixture.buy_hot_instruction.data.len(),
            HOT_EXECUTION_ENVELOPE_BYTES_V3 + DIRECT_REGISTRATION_REQUEST_BYTES_V3
        );
        assert_ne!(fixture.capability_seals[0], fixture.capability_seals[1]);
        assert_ne!(fixture.registered_records[0], fixture.registered_records[1]);
        assert_ne!(fixture.maker_replays[0], fixture.maker_replays[1]);
        assert_eq!(
            fixture.root_poststates[0].len(),
            CAPABILITY_ROOT_HEADER_BYTES_V1
                + dclutch_trading::successor::DIRECT_ROOT_STATE_BYTES_V1
        );
        assert_eq!(
            fixture.root_poststates[0][..CAPABILITY_ROOT_HEADER_BYTES_V1],
            fixture.root_poststates[1][..CAPABILITY_ROOT_HEADER_BYTES_V1]
        );
        assert!(fixture.reserved_claims > 0);
        assert!(fixture.reserved_collateral > 0);
        assert!(
            fixture
                .accounts
                .iter()
                .any(|value| value.key == fixture.custody_replay)
        );
        assert!(
            fixture
                .accounts
                .iter()
                .any(|value| value.key == fixture.custody_vault)
        );
        let mut keys = fixture
            .accounts
            .iter()
            .map(|value| value.key)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), fixture.accounts.len());
    }

    /// The live RentCredit the adapter authenticates: one credit for the whole
    /// Market lifecycle, 128 canonical V2 bytes, rent-exempt at that width,
    /// bound to the executing Market/release-set/generation, and a PDA of the
    /// Rent program that the frame also carries as an executable coordinate.
    #[test]
    fn the_frame_carries_one_v2_lifecycle_credit_and_its_rent_program() {
        let input = input();
        let rent = Rent::default();
        let artifacts =
            build_direct_hot_artifact_fixture_v5(input.deployment_widths, input.geometry)
                .expect("artifacts");
        let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
            .expect("config")
            .encode();
        let product = product_fixture(input).expect("product");
        let manifest = capability_manifest(input, &artifacts, &config).expect("manifest");
        let market = market_and_claims(input, &product, &manifest, &rent)
            .expect("state")
            .market;
        let fixture = build_direct_hot_chain_fixture_v5(input).expect("chain fixture");
        let (credit_key, credit) =
            lifecycle_rent_credit(input.rent_program, market, input.release_set, input.payer)
                .expect("credit");
        let account = fixture
            .accounts
            .iter()
            .find(|value| value.key == credit_key)
            .expect("lifecycle RentCredit account");
        assert_eq!(account.account.owner, input.rent_program);
        assert!(!account.account.executable);
        assert_eq!(account.account.data.len(), LIFECYCLE_RENT_CREDIT_BYTES_V2);
        assert_eq!(account.account.data, credit.to_bytes().to_vec());
        assert!(
            rent.is_exempt(account.account.lamports, LIFECYCLE_RENT_CREDIT_BYTES_V2),
            "credit must be rent-exempt at its exact width"
        );
        assert_eq!(credit.market().to_bytes(), market.to_bytes());
        assert_eq!(credit.release_set().to_bytes(), input.release_set);
        assert_eq!(credit.generation(), GENERATION);
        assert_eq!(
            Pubkey::create_program_address(
                &[
                    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                    market.as_ref(),
                    &GENERATION.to_le_bytes(),
                    &[credit.pda_seeds().bump()],
                ],
                &input.rent_program,
            )
            .expect("credit PDA"),
            credit_key
        );
        let rent_program = fixture
            .accounts
            .iter()
            .find(|value| value.key == input.rent_program)
            .expect("Rent program account");
        assert!(rent_program.account.executable);
        // Exactly one credit: the second per-authority V1 credit is gone.
        assert_eq!(
            fixture
                .accounts
                .iter()
                .filter(
                    |value| value.account.owner == input.rent_program && !value.account.executable
                )
                .count(),
            1
        );
    }

    /// The live account the Hot executor's Custody role lookup consumes.
    ///
    /// `selected_role_program_v3` scans the downgraded effect accounts for the
    /// program the activated release set names for the role and accepts it only
    /// if it is unique, executable, not a signer and not writable. The four
    /// Custody routes had no such account at any coordinate, so every one of
    /// them refused before its first CPI; the Claims route found its own callee
    /// inside its frame and did not.
    #[test]
    fn the_topology_carries_the_custody_program_the_custody_routes_invoke() {
        let input = input();
        let fixture = build_direct_hot_chain_fixture_v5(input).expect("chain fixture");
        let matches = fixture
            .accounts
            .iter()
            .filter(|value| value.key == input.custody_program)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "the role lookup refuses a second match");
        let custody = matches.first().expect("Custody program account");
        assert!(custody.account.executable);
        assert!(!custody.snapshot_for_rollback);
        let meta = fixture
            .hot_instruction
            .accounts
            .iter()
            .filter(|value| value.pubkey == input.custody_program)
            .collect::<Vec<_>>();
        assert_eq!(meta.len(), 1);
        let meta = meta.first().expect("Custody program meta");
        assert!(!meta.is_signer);
        assert!(!meta.is_writable);
        // ProgramTest owns the real deployed record for this key, exactly as it
        // does for Claims and Core, so the fixture must hand it over rather than
        // installing a placeholder over the deployment.
        assert!(
            fixture
                .externally_installed_keys
                .contains(&input.custody_program)
        );
        // Each of the four Custody routes selects the same one program, and it
        // is not the Claims callee.
        assert_ne!(input.custody_program, input.claims_program);
        assert!(
            fixture
                .accounts
                .iter()
                .any(|value| value.key == input.claims_program && value.account.executable)
        );
    }

    /// The four Custody `CallerAuthority` coordinates are PDAs of their routes.
    ///
    /// Three of them were literal keys and the fourth was derived with the
    /// family request digest where `custody_composition_v3::prepare` uses the
    /// child request's own `context`. Distinctness alone is what
    /// `validate_accounts` asks and it is what a placeholder already satisfied,
    /// so the property that has to be pinned is that each coordinate is the
    /// address the Hot executor will derive for that route: off the Trading
    /// program, seeded by the release set, the Market, the Trading role, the
    /// buyer maker root, and the SHA-256 of that route's projected request.
    #[test]
    fn each_custody_route_authority_is_the_pda_the_hot_executor_derives() {
        let input = input();
        let fixture = build_direct_hot_chain_fixture_v5(input).expect("chain fixture");
        let routes = fixture.custody_routes;
        for (index, route) in routes.iter().enumerate() {
            assert!(
                !routes
                    .iter()
                    .take(index)
                    .any(|earlier| earlier.authority == route.authority),
                "route {index} repeats an earlier route's caller authority"
            );
            // A PDA is off the ed25519 curve. Three of these coordinates were
            // literal keys and could never satisfy that.
            assert!(
                !route.authority.is_on_curve(),
                "route {index} is not a program address at all"
            );
            // Every authority is a real installed frame coordinate.
            assert!(
                fixture
                    .accounts
                    .iter()
                    .any(|value| value.key == route.authority),
                "route {index} authority is not an account in the frame"
            );
        }

        // The seed rule, stated as the refusal it replaces: the context seed is
        // the child request's own `context` -- the buyer maker root -- and the
        // family request digest enters only through the request hash. Deriving
        // the authority the way the fixture used to must produce a DIFFERENT
        // address, or this test would pass over the bug it exists for.
        let request = fixture
            .hot_instruction
            .data
            .get(HOT_EXECUTION_ENVELOPE_BYTES_V3..)
            .expect("family request");
        // The buyer maker root: the `context` every projected Custody request
        // carries, and therefore the fourth seed of every one of these PDAs.
        let context = fixture.maker_replays[1].to_bytes();
        let derive = |context: [u8; 32], digest: [u8; 32]| {
            CallerAuthoritySeedsV1::new(
                core_content(input.release_set).expect("release set"),
                fixture.market.to_bytes(),
                ExecutionRoleV1::Trading,
                context,
                digest,
            )
            .map(|seeds| Pubkey::find_program_address(&seeds.as_slices(), &input.trading_program).0)
        };

        for (index, route) in routes.iter().enumerate() {
            // Positive: the reported address IS the context-seeded derivation,
            // reproduced here from public fields only.
            assert_eq!(
                derive(context, route.request_digest).expect("context-seeded derivation"),
                route.authority,
                "route {index} is not the context-seeded address"
            );
            // Negative, and the sharpest one available: the derivation the
            // fixture actually used before this fix -- the FAMILY request
            // digest in the context seed, this route's own child-request hash
            // still in the fifth. It is a well-formed seed order, so nothing
            // refuses it; it simply lands somewhere else.
            let hostile = derive(hash(request).to_bytes(), route.request_digest)
                .expect("family-seeded derivation is well-formed");
            assert!(
                !routes.iter().any(|value| value.authority == hostile),
                "route {index} was seeded by the family request, not by its own context"
            );
        }

        // A zero context or a zero request digest is not a wrong address, it is
        // no address: `CallerAuthoritySeedsV1` refuses the seed order outright,
        // so neither can be what any coordinate holds.
        for (context, digest) in [
            ([0; 32], hash(request).to_bytes()),
            (context, [0; 32]),
            ([0; 32], [0; 32]),
        ] {
            assert!(
                derive(context, digest).is_err(),
                "a zero caller-authority coordinate was accepted"
            );
        }
    }

    #[test]
    fn maker_alias_and_zero_release_refuse() {
        let mut value = input();
        value.makers[1] = value.makers[0];
        assert_eq!(
            build_direct_hot_chain_fixture_v5(value),
            Err(DirectHotChainFixtureErrorV5::Input)
        );
        let mut value = input();
        value.release_set = [0; 32];
        assert_eq!(
            build_direct_hot_chain_fixture_v5(value),
            Err(DirectHotChainFixtureErrorV5::Input)
        );
    }

    /// The lookup-table address set has ONE author, and the copies are compared.
    ///
    /// The waist carried its own derivation until 2026-09-02. It filtered
    /// `meta.is_signer` PER OCCURRENCE; the operator's filters the signer KEY
    /// SET. They differ exactly when an account signs in one instruction of a
    /// batch and not in another, because the per-occurrence filter then admits a
    /// signer into the table -- and a v0 message must carry every signer as a
    /// static key.
    ///
    /// This drives both on the real registered Sell and Buy frames, so whether
    /// they agree TODAY is measured rather than assumed, and the day a frame
    /// makes them disagree this case names it instead of the message quietly
    /// gaining a non-static signer.
    #[test]
    fn canonical_lookup_addresses_disagreement_is_measured_not_assumed() {
        let input = input();
        let fixture =
            build_direct_registered_creation_chain_fixture_v4(input).expect("registered chain");
        let payer = input.payer;

        for (side, instruction) in [
            ("Sell", &fixture.sell_hot_instruction),
            ("Buy", &fixture.buy_hot_instruction),
        ] {
            let batch = [instruction.clone()];
            let authored = crate::waist::canonical_lookup_addresses(&batch, payer);
            let superseded =
                crate::waist::superseded_per_occurrence_lookup_addresses(&batch, payer);

            // Sorted, deduplicated, and never empty -- the operator refuses an
            // empty or over-256 table rather than compiling one.
            assert!(!authored.is_empty(), "{side}: an empty table is refused");
            assert!(authored.len() <= 256, "{side}: table over the index space");
            let mut sorted = authored.clone();
            sorted.sort_unstable_by_key(Pubkey::to_bytes);
            sorted.dedup();
            assert_eq!(authored, sorted, "{side}: table is not canonical");

            // No signer may appear in the table, by KEY. This is the property the
            // superseded copy could violate, and it is asserted directly rather
            // than inferred from the two agreeing.
            for meta in instruction.accounts.iter().filter(|meta| meta.is_signer) {
                assert!(
                    !authored.contains(&meta.pubkey),
                    "{side}: signer {} is in the lookup table",
                    meta.pubkey
                );
            }
            assert!(
                !authored.contains(&payer),
                "{side}: the fee payer is in the lookup table"
            );

            // And the measurement: on THIS frame the two agree. That is a fact
            // about these fixtures, not about the derivations.
            assert_eq!(
                authored, superseded,
                "{side}: the two derivations disagree on this frame -- the \
                 superseded copy is the one that may admit a signer, so read \
                 the difference before trusting either",
            );
        }
    }

    /// Execute both registered creation profiles against a real frame.
    ///
    /// The registered creation artifacts were proved to agree with themselves
    /// and never RUN: an `AccountProfileV2` is a declaration, and a component
    /// test that checks its encoding and digest passes forever over a conjunct
    /// no account can satisfy. This runs the declaration.
    ///
    /// What it found, and what the chain campaign met as its fourth wall: the
    /// Buy profile carried two Custody-window `require_key` operations naming
    /// identity registers 24 and 25, and those registers are written by
    /// `project_identity` from the Realm record IN THE SAME PASS.
    /// `apply_operations` evaluates every `require_*` against the INPUT bank
    /// (`v2.rs::project_atomic` hands it `registers.input_identities`), so both
    /// conjuncts compared a real key against thirty-two zero bytes and were
    /// unsatisfiable by any transaction -- measured on real ELFs as
    /// `IdentityMismatch` at coordinate 34, 308,354 CU, before any child CPI.
    /// The Sell profile carried neither, and its three `require_*` operations
    /// all name trusted-environment registers that ARE seeded before the pass --
    /// which is exactly why a registered Sell executed behind the action-gate
    /// probe and a Buy did not.
    ///
    /// Both are gone. They were never a gate: Custody authenticates the Realm
    /// record itself and requires `request.mint == realm.collateral_mint()` and
    /// the live frame mint to equal `request.mint`, which is strictly stronger
    /// than a restatement in the caller's profile, and is the same decision
    /// `ordinary_account_artifacts_v3::operations` records for the inline
    /// family. What this case now asserts is the positive half: the pass writes
    /// the REALM's collateral mint and token program into the two output
    /// registers the Effect copies into `CustodyRequestLayoutV1`, and it FOLLOWS
    /// the Realm record rather than authenticating it -- so the perturbation
    /// below moves the projected register, which is the exact reason the law
    /// has to live in Custody and not here.
    ///
    /// The frame corrections below are not fixture repairs. Four coordinates
    /// hold accounts the fixture deliberately defers to the harness
    /// (`externally_installed_keys`), and its stubs for those elide width and
    /// balance; `creation_case` and `add_release_waist` install the real ones.
    /// A host run over the stubs would measure the stub, not the chain.
    #[test]
    #[allow(clippy::indexing_slicing, clippy::too_many_lines)]
    fn both_registered_creation_profiles_project_a_real_frame() {
        use dclutch_market::capability_program::hot_v3::HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3;
        use dclutch_market::realm::RealmLayoutV1;
        use dclutch_trading::registered_creation_artifacts_v4::{
            DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4,
            DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4, REGISTERED_IDENTITY_MINT_V4,
            REGISTERED_IDENTITY_TOKEN_PROGRAM_V4,
        };
        use dclutch_vm::account_profile::{
            AccountObservationV1,
            v2::{
                PhysicalAccountDataGeometryV2, ProjectionRegistersV2, TrustedEnvironmentV2,
                project_atomic,
            },
        };

        let input = input();
        let rent = Rent::default();
        let config_value =
            DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
                .expect("config");
        let config = config_value.encode();
        let product = product_fixture(input).expect("product");
        let artifacts = registered_creation_artifacts_v4(input).expect("artifacts");
        let manifest =
            capability_manifest_selected(input, artifacts.sell(), &config).expect("manifest");
        let state = market_and_claims(input, &product, &manifest, &rent).expect("state");
        let capability = capability_fixture(input, manifest, state.market).expect("capability");
        let requests = registered_creation_requests_v4(
            input,
            &rent,
            config_value,
            &state,
            &product,
            &capability,
        )
        .expect("requests");
        let custody =
            registered_buy_custody_v4(input, &rent, &product, &state, &requests).expect("custody");
        let tail = input.geometry.outcome_count();
        let projected_keys = [
            hash(&config).to_bytes(),
            product.product.digest,
            product.portfolio.digest,
            product.basis.digest,
        ];

        // Build one side's observation bank exactly as `hot_v3` presents it:
        // the shared runtime prefix carries content digests as projection keys
        // (`logical_projection_key_v3`), and coordinate 4 is the variable-width
        // linked basis.
        let observe = |accounts: &[ChainAccount]| {
            accounts
                .iter()
                .enumerate()
                .map(|(coordinate, account)| {
                    let key = match coordinate {
                        1..=4 => projected_keys[coordinate - 1],
                        _ => account.key.to_bytes(),
                    };
                    (key, account.account.owner.to_bytes())
                })
                .collect::<Vec<_>>()
        };
        let project = |profile: AccountProfileV2<'_>,
                       accounts: &[ChainAccount],
                       views: &[([u8; 32], [u8; 32])]| {
            // Privileges come from the profile, exactly as `pack_runtime`
            // derives the packed AccountMetas the instruction carries: the two
            // payer coordinates sign, and writability is the profile's.
            let signers = [accounts[6].key, accounts[9].key];
            let observations = views
                .iter()
                .zip(accounts)
                .enumerate()
                .map(|(coordinate, ((key, owner), account))| {
                    let ordinal = profile
                        .physical_account_ordinal_with_dynamic_spans(tail, &[], coordinate)
                        .expect("ordinal");
                    let writable = profile
                        .physical_account_geometry_with_dynamic_spans(tail, &[], ordinal)
                        .expect("geometry")
                        .privileges()
                        .writable();
                    let signer = signers.contains(&account.key);
                    if coordinate == 4 {
                        AccountObservationV1::new_adapter_authenticated_variable_data(
                            key,
                            owner,
                            account.account.lamports,
                            &account.account.data,
                            signer,
                            writable,
                            account.account.executable,
                        )
                    } else {
                        AccountObservationV1::new(
                            key,
                            owner,
                            account.account.lamports,
                            account.account.data.as_slice(),
                            signer,
                            writable,
                            account.account.executable,
                        )
                    }
                })
                .collect::<Vec<_>>();
            let mut input_scalars = vec![0_u64; DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4];
            let mut input_identities =
                vec![[0_u8; 32]; DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4];
            input_identities[HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3] = [0x5a; 32];
            if let TrustedEnvironmentV2::CurrentSlot { destination } = profile.trusted_environment()
            {
                input_scalars[usize::from(destination)] = input.clock_slot;
            }
            if let Some(destination) = profile.trusted_current_executing_program_identity() {
                input_identities[usize::from(destination)] = input.trading_program.to_bytes();
            }
            if let Some(destination) = profile.trusted_system_program_identity() {
                input_identities[usize::from(destination)] = system_program::ID.to_bytes();
            }
            // Nothing else is seeded. The input bank a family may fill is
            // exactly what `seed_trusted_environment_v3` fills on chain: the
            // parent request digest, the current slot, the current executing
            // program and the System Program. Every other register in this pass
            // is the pass's own output.
            let mut scratch_scalars = vec![0_u64; DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4];
            let mut scratch_identities =
                vec![[0_u8; 32]; DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4];
            let mut output_scalars = vec![0_u64; DIRECT_REGISTERED_CREATION_COMMON_SCALARS_V4];
            let mut output_identities =
                vec![[0_u8; 32]; DIRECT_REGISTERED_CREATION_COMMON_IDENTITIES_V4];
            project_atomic(
                profile,
                tail,
                &observations,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
                None,
            )
            .map(|()| output_identities)
        };

        // The Sell: thirteen coordinates, three `require_*` operations, every
        // one of them against a trusted-environment register. It projects.
        let sell_profile =
            AccountProfileV2::decode(&artifacts.sell.account_profile).expect("Sell profile");
        let sell = registered_creation_logical_accounts_v4(
            input,
            &rent,
            artifacts.sell(),
            &config,
            &product,
            &state,
            &capability,
            &requests,
            Some(&custody),
        )
        .expect("Sell logical accounts");
        let sell_views = observe(&sell);
        assert!(
            project(sell_profile, &sell, &sell_views).is_ok(),
            "the registered Sell profile must project its own frame",
        );

        // The Buy: fifty-six coordinates, and four of them are the harness's.
        let mut buy = registered_creation_logical_accounts_v4(
            input,
            &rent,
            artifacts.buy(),
            &config,
            &product,
            &state,
            &capability,
            &requests,
            Some(&custody),
        )
        .expect("Buy logical accounts");
        let cache_bytes = dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1;
        let loader_bytes = dclutch_registry::svm::LOADER_V3_PROGRAM_BYTES;
        for (coordinate, bytes) in [
            (14_usize, cache_bytes),
            (27, cache_bytes),
            (43, cache_bytes),
            (15, loader_bytes),
            (28, loader_bytes),
            (44, loader_bytes),
            (16, loader_bytes),
            (29, loader_bytes),
            (45, loader_bytes),
            (23, 17),
            (40, 17),
        ] {
            let account = buy.get_mut(coordinate).expect("deferred coordinate");
            account.account.data = vec![0_u8; bytes];
            account.account.lamports = rent.minimum_balance(bytes);
        }

        // Every declared width is satisfied. There is no width defect on this
        // side: the one convicted coordinate pair below is an identity.
        let buy_profile =
            AccountProfileV2::decode(&artifacts.buy.account_profile).expect("Buy profile");
        for (coordinate, account) in buy.iter().enumerate() {
            let ordinal = buy_profile
                .physical_account_ordinal_with_dynamic_spans(tail, &[], coordinate)
                .expect("ordinal");
            let observed = account.account.data.len();
            let satisfied = match buy_profile
                .physical_account_geometry_with_dynamic_spans(tail, &[], ordinal)
                .expect("geometry")
                .data()
            {
                PhysicalAccountDataGeometryV2::Exact { bytes } => observed == bytes,
                PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
                    observed == 0 || observed == live_bytes
                }
                PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                    observed >= minimum_bytes
                }
                PhysicalAccountDataGeometryV2::Opaque => true,
            };
            assert!(
                satisfied,
                "Buy coordinate {coordinate} observed {observed} bytes against its declaration",
            );
        }

        // WALL #4, repaired. The Buy now projects its own honest frame, and the
        // two registers the Effect copies into `CustodyRequestLayoutV1::{MINT,
        // TOKEN_PROGRAM}` carry the Realm's own facts rather than zero.
        let buy_views = observe(&buy);
        let projected = project(buy_profile, &buy, &buy_views)
            .expect("the registered Buy profile must project its own honest frame");
        assert_eq!(
            projected[REGISTERED_IDENTITY_MINT_V4],
            custody.realm.mint.to_bytes(),
            "register 24 must carry the Realm record's collateral mint",
        );
        assert_eq!(
            projected[REGISTERED_IDENTITY_TOKEN_PROGRAM_V4],
            custody.realm.token_program.to_bytes(),
            "register 25 must carry the Realm record's token program",
        );

        // The control that keeps the delegation honest. This pass FOLLOWS the
        // Realm record; it does not authenticate it. Rewrite the collateral-mint
        // field of the Realm account at coordinate 18 and the projected register
        // moves with it, with no refusal anywhere in the profile -- which is the
        // reason the law is Custody's: `authenticate_realm` re-derives the
        // Registry record address from the signed content digest, so a Realm
        // whose bytes were rewritten is a Realm whose digest no longer resolves.
        // The Realm record is a route alias at coordinates 31 and 47, and an
        // alias compares DATA as well as key, so a perturbation that rewrote one
        // copy would measure `AliasMismatch` and never reach the projection.
        let mut perturbed = buy.clone();
        let forged = key(0xd7).to_bytes();
        let realm_key = perturbed[18].key;
        for account in &mut perturbed {
            if account.key == realm_key {
                account.account.data
                    [RealmLayoutV1::COLLATERAL_MINT..RealmLayoutV1::COLLATERAL_MINT + 32]
                    .copy_from_slice(&forged);
            }
        }
        let perturbed_views = observe(&perturbed);
        let perturbed_registers = project(buy_profile, &perturbed, &perturbed_views)
            .expect("the profile authenticates no Realm byte and must still project");
        assert_eq!(
            perturbed_registers[REGISTERED_IDENTITY_MINT_V4], forged,
            "the projection must follow the Realm record, proving the pass is connected",
        );
        assert_ne!(
            perturbed_registers[REGISTERED_IDENTITY_MINT_V4],
            projected[REGISTERED_IDENTITY_MINT_V4],
            "a perturbation that does not move the register would be measuring nothing",
        );
    }
}

fn finalized(owner: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> Finalized {
    let digest = hash(&bytes).to_bytes();
    let raw = Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &owner).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &owner).0;
    Finalized {
        raw,
        staging,
        bytes,
        digest,
        schema,
        owner,
    }
}

fn product_content(value: [u8; 32]) -> Result<ProductContentId, DirectHotChainFixtureErrorV5> {
    ProductContentId::new(value).map_err(|_| DirectHotChainFixtureErrorV5::Input)
}

fn core_identity(value: [u8; 32]) -> Result<CoreIdentity, DirectHotChainFixtureErrorV5> {
    CoreIdentity::new(value).map_err(|_| DirectHotChainFixtureErrorV5::Input)
}

fn key(value: u8) -> Pubkey {
    Pubkey::new_from_array([value; 32])
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), DirectHotChainFixtureErrorV5> {
    let destination = output
        .get_mut(offset..offset.saturating_add(value.len()))
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    destination.copy_from_slice(value);
    Ok(())
}

// Remaining helpers own manifest/root construction, account frames, and
// Profile13 physical packing.

/// Build the same canonical Direct fixture through the family-generic
/// artifact-derived bundle builder.
///
/// The hand-built path above states the whole topology; this path states only
/// the corpus — the request, the semantic prestate accounts, and the waist —
/// and lets the builder derive record addresses, the seal, packing,
/// privileges, funding, the lifecycle-created maker replays, and all five
/// caller authorities by executing the emitted artifacts host-side. The two
/// paths agreeing byte-for-byte is the reproduction gate for the builder.
pub mod via_builder {
    use dclutch_chain_bundle_builder::{
        WaistFactsV1,
        artifacts::{ArtifactSetV1, derive_record},
        bundle::{BundleInputV1, FixedCorpusV1, ScenarioV1, build_bundle},
        frame::{
            BuiltAccountV1, data_account, external_with_view, program, program_with_deployed_view,
            program_with_view, rent_sysvar_bytes, vacant,
        },
    };
    use dclutch_market::realm::REALM_SCHEMA_RELEASE_ID_V1;
    use dclutch_product::admission::{
        PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
    };
    use dclutch_trading::native_evidence_v3::{
        DIRECT_NATIVE_EVIDENCE_BYTES_V3,
        encode_direct_headerless_registry_native_evidence_v4_atomic,
    };

    use super::*;

    /// Bundle-builder reproduction of [`build_direct_hot_chain_fixture_v5`].
    #[allow(clippy::too_many_lines)]
    pub fn build_direct_hot_chain_fixture_via_builder_v1(
        input: DirectHotChainInputV5,
    ) -> Result<DirectHotChainFixtureV5, DirectHotChainFixtureErrorV5> {
        validate_input(input)?;
        let rent = Rent::default();
        let artifacts =
            build_direct_hot_artifact_fixture_v5(input.deployment_widths, input.geometry)
                .map_err(DirectHotChainFixtureErrorV5::Artifact)?;
        let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
            .encode();
        let product = product_fixture(input)?;
        let manifest = capability_manifest(input, &artifacts, &config)?;
        let state = market_and_claims(input, &product, &manifest, &rent)?;
        let intents = intents(input, state.market, product.collateral_accounts)?;
        let request = direct_request(input.makers, intents)?;
        let capability = capability_fixture(input, manifest, state.market)?;
        let realm = realm_fixture(
            input,
            product.collateral_accounts,
            state.market,
            capability.buyer_maker,
        )?;

        // The SAME call the hand-built path makes, so the two routes cannot
        // report different bumps for one address -- which is the property
        // `builder_reproduces_the_hand_built_direct_fixture` exists to hold.
        let claims_caller_authority =
            claims_caller_authority_v5(input, &product, &state, &request)?;

        let waist = WaistFactsV1 {
            registry_program: input.registry_program,
            trading_program: input.trading_program,
            core_program: input.core_program,
            claims_program: input.claims_program,
            custody_program: input.custody_program,
            release_set: input.release_set,
            activation_cache: input.activation_cache,
            trading_semantic_release: input.trading_semantic_release,
        };
        let set = ArtifactSetV1 {
            descriptor: &artifacts.bundle.descriptor,
            account_profile: &artifacts.bundle.account_profile,
            request_profile: &artifacts.bundle.request_profile,
            transition: &artifacts.bundle.transition,
            effect: &artifacts.bundle.effect,
            lifecycle: &artifacts.bundle.lifecycle_policy,
            strategy: &artifacts.bundle.strategy,
            program_set: &artifacts.program_set,
            manifest: &capability.manifest,
            config: &config,
        };

        // The Ed25519 evidence at build time: exact offsets and maker public
        // keys over the exact nested instruction bytes; signatures are a
        // nonzero placeholder (the encoder refuses all-zero evidence). Only
        // offsets and keys seed registers — the native program checks
        // signatures at submission, over evidence the campaign re-encodes with
        // the real ones.
        let envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(request.len()).map_err(|_| DirectHotChainFixtureErrorV5::Input)?,
            input.release_set,
            state.market.to_bytes(),
            GENERATION,
            hash(&capability.root_bytes).to_bytes(),
        )
        .map_err(|_| {
            #[cfg(test)]
            std::eprintln!("via_builder: envelope refused");
            DirectHotChainFixtureErrorV5::Encoding
        })?;
        let mut instruction_data =
            Vec::with_capacity(HOT_EXECUTION_ENVELOPE_BYTES_V3 + request.len());
        instruction_data.extend_from_slice(&envelope.to_bytes());
        instruction_data.extend_from_slice(&request);
        let mut evidence = [0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
        encode_direct_headerless_registry_native_evidence_v4_atomic(
            2,
            &instruction_data,
            [[1_u8; 64]; 2],
            &mut evidence,
        )
        .map_err(|_| {
            #[cfg(test)]
            std::eprintln!("via_builder: ed25519 evidence refused");
            DirectHotChainFixtureErrorV5::Encoding
        })?;

        let fixed = FixedCorpusV1 {
            market: data_account(
                &rent,
                state.market,
                input.core_program,
                state.core_bytes.clone(),
            ),
            root: data_account(
                &rent,
                capability.root,
                input.trading_program,
                capability.root_bytes.clone(),
            ),
            product: derive_record(
                input.registry_program,
                PRODUCT_RECORD_SCHEMA_ID_V2,
                &product.product.bytes,
            ),
            result_domain: derive_record(
                input.registry_program,
                RESULT_DOMAIN_SCHEMA_ID_V2,
                &product.domain.bytes,
            ),
            portfolio: derive_record(
                input.registry_program,
                PORTFOLIO_SCHEMA_ID_V2,
                &product.portfolio.bytes,
            ),
            linked_basis: derive_record(
                input.registry_program,
                GRADED_BASIS_RECORD_SCHEMA_ID_V3,
                &product.basis.bytes,
            ),
            core_programdata: input.core_programdata,
            trading_programdata: input.trading_programdata,
        };
        let realm_record = derive_record(
            input.registry_program,
            REALM_SCHEMA_RELEASE_ID_V1,
            &realm.realm.bytes,
        );
        let (credit_key, credit) = lifecycle_rent_credit(
            input.rent_program,
            state.market,
            input.release_set,
            input.payer,
        )?;
        let (source_balance, allowance) = source_collateral(input.trade)?;

        // The Direct corpus: every runtime self-coordinate the artifacts do
        // not determine. Maker replays (5, 8) and the five caller authorities
        // (12, 34, 48, 62, 76) are deliberately absent — the builder derives
        // them.
        let bindings: Vec<(usize, BuiltAccountV1)> = vec![
            (
                6,
                vacant(input.payer).with_observed(solana_account::Account {
                    // `direct_case` funds the payer to ten SOL; the chain view
                    // must show a balance the Create plans can debit.
                    lamports: 10_000_000_000,
                    data: Vec::new(),
                    owner: system_program::ID,
                    executable: false,
                    rent_epoch: 0,
                }),
            ),
            (
                7,
                data_account(
                    &rent,
                    credit_key,
                    input.rent_program,
                    credit.to_bytes().to_vec(),
                ),
            ),
            (
                9,
                vacant(input.payer).with_observed(solana_account::Account {
                    lamports: 10_000_000_000,
                    data: Vec::new(),
                    owner: system_program::ID,
                    executable: false,
                    rent_epoch: 0,
                }),
            ),
            (10, program(input.rent_program)),
            (11, program(system_program::ID)),
            (
                13,
                data_account(
                    &rent,
                    state.claims_market,
                    input.claims_program,
                    state.claims_bytes.clone(),
                ),
            ),
            (
                14,
                data_account(
                    &rent,
                    fixed.linked_basis.raw,
                    input.registry_program,
                    fixed.linked_basis.bytes.clone(),
                ),
            ),
            (15, vacant(fixed.linked_basis.staging)),
            (
                16,
                data_account(
                    &rent,
                    fixed.product.raw,
                    input.registry_program,
                    fixed.product.bytes.clone(),
                ),
            ),
            (17, vacant(fixed.product.staging)),
            (
                18,
                data_account(
                    &rent,
                    fixed.result_domain.raw,
                    input.registry_program,
                    fixed.result_domain.bytes.clone(),
                ),
            ),
            (19, vacant(fixed.result_domain.staging)),
            (
                20,
                data_account(
                    &rent,
                    fixed.portfolio.raw,
                    input.registry_program,
                    fixed.portfolio.bytes.clone(),
                ),
            ),
            (21, vacant(fixed.portfolio.staging)),
            (
                22,
                external_with_view(sysvar::rent::ID, sysvar::ID, rent_sysvar_bytes(&rent)),
            ),
            (
                23,
                data_account(
                    &rent,
                    state.market,
                    input.core_program,
                    state.core_bytes.clone(),
                ),
            ),
            (
                24,
                external_with_view(
                    input.activation_cache,
                    input.registry_program,
                    vec![0; dclutch_registry::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1],
                ),
            ),
            (25, program_with_deployed_view(input.registry_program)),
            (
                26,
                program_with_view(input.trading_program, input.trading_programdata),
            ),
            (
                27,
                external_with_view(
                    input.trading_programdata,
                    bpf_loader_upgradeable::ID,
                    vec![
                        0;
                        usize::try_from(input.deployment_widths.trading_programdata_bytes)
                            .map_err(|_| DirectHotChainFixtureErrorV5::Input)?
                    ],
                ),
            ),
            (
                28,
                program_with_view(input.claims_program, input.claims_programdata),
            ),
            (
                29,
                external_with_view(
                    input.claims_programdata,
                    bpf_loader_upgradeable::ID,
                    vec![
                        0;
                        usize::try_from(input.deployment_widths.claims_programdata_bytes)
                            .map_err(|_| DirectHotChainFixtureErrorV5::Input)?
                    ],
                ),
            ),
            (
                30,
                program_with_view(input.core_program, input.core_programdata),
            ),
            (
                31,
                external_with_view(
                    input.core_programdata,
                    bpf_loader_upgradeable::ID,
                    vec![
                        0;
                        usize::try_from(input.deployment_widths.core_programdata_bytes)
                            .map_err(|_| DirectHotChainFixtureErrorV5::Input)?
                    ],
                ),
            ),
            (
                32,
                data_account(
                    &rent,
                    state.positions[0].0,
                    input.claims_program,
                    state.positions[0].1.clone(),
                ),
            ),
            (
                33,
                data_account(
                    &rent,
                    state.positions[1].0,
                    input.claims_program,
                    state.positions[1].1.clone(),
                ),
            ),
            (
                40,
                data_account(
                    &rent,
                    realm_record.raw,
                    input.registry_program,
                    realm_record.bytes.clone(),
                ),
            ),
            (41, vacant(realm_record.staging)),
            (
                42,
                data_account(
                    &rent,
                    realm.custody_replay,
                    input.custody_program,
                    realm.custody_replay_bytes.clone(),
                ),
            ),
            (
                43,
                data_account(
                    &rent,
                    realm.mint,
                    realm.token_program,
                    mint_bytes(mint_supply(input.trade)),
                ),
            ),
            (
                44,
                data_account(
                    &rent,
                    product.collateral_accounts[0],
                    realm.token_program,
                    token_bytes(
                        realm.mint,
                        input.makers[1],
                        source_balance,
                        Some(realm.custody_authority),
                        allowance,
                    )?,
                ),
            ),
            (
                45,
                data_account(
                    &rent,
                    product.collateral_accounts[1],
                    realm.token_program,
                    token_bytes(
                        realm.mint,
                        input.makers[0],
                        SELLER_COLLATERAL_BALANCE,
                        None,
                        0,
                    )?,
                ),
            ),
            (46, vacant(realm.custody_authority)),
            (47, program_with_deployed_view(realm.token_program)),
            (
                73,
                data_account(
                    &rent,
                    product.collateral_accounts[2],
                    realm.token_program,
                    token_bytes(realm.mint, input.payer, FEE_COLLATERAL_BALANCE, None, 0)?,
                ),
            ),
            (
                usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3),
                program(input.custody_program),
            ),
        ];

        let externally_installed_extra = [
            input.claims_programdata,
            Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
        ];
        let scenario = ScenarioV1 {
            family_request: &request,
            tail_count: input.geometry.outcome_count(),
            clock_slot: input.clock_slot,
            generation: GENERATION,
            ed25519_evidence: Some(&evidence),
            native_message_instruction_index: 2,
            externally_installed_extra: &externally_installed_extra,
            payer: input.payer,
        };
        let bundle = build_bundle(&BundleInputV1 {
            set,
            waist,
            scenario,
            fixed,
            bindings: &bindings,
            rent: &rent,
        })
        .map_err(|error| {
            // Surface the builder's stage name during reproduction work.
            #[cfg(test)]
            std::eprintln!("bundle builder refused: {error:?}");
            let _ = error;
            DirectHotChainFixtureErrorV5::Profile
        })?;

        // Reassemble the reference fixture shape from the builder's output.
        let seller_maker = bundle
            .logical
            .get(5)
            .ok_or(DirectHotChainFixtureErrorV5::Profile)?
            .key;
        let buyer_maker = bundle
            .logical
            .get(8)
            .ok_or(DirectHotChainFixtureErrorV5::Profile)?
            .key;
        let mut custody_routes = [CustodyRouteAuthorityV3 {
            authority: Pubkey::default(),
            request_digest: [0; 32],
        }; 4];
        for (slot, coordinate) in custody_routes.iter_mut().zip([34_usize, 48, 62, 76]) {
            let authority = bundle
                .authorities
                .iter()
                .find(|value| value.coordinate == coordinate)
                .ok_or_else(|| {
                    #[cfg(test)]
                    std::eprintln!(
                        "via_builder: no authority at coordinate {coordinate}; derived {:?}",
                        bundle
                            .authorities
                            .iter()
                            .map(|value| value.coordinate)
                            .collect::<Vec<_>>()
                    );
                    DirectHotChainFixtureErrorV5::Profile
                })?;
            *slot = CustodyRouteAuthorityV3 {
                authority: authority.authority,
                request_digest: authority.request_digest,
            };
        }
        let capability_seal_accounts = bundle
            .hot_instruction
            .accounts
            .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
            .ok_or(DirectHotChainFixtureErrorV5::Profile)?
            .to_vec();
        let mut hot_instruction = bundle.hot_instruction.clone();
        alias_sealed_execution_metas(&mut hot_instruction.accounts)?;
        Ok(DirectHotChainFixtureV5 {
            hot_instruction,
            capability_seal_accounts,
            signed_messages: [
                intents[0]
                    .signed_preimage()
                    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
                intents[1]
                    .signed_preimage()
                    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
            ],
            accounts: bundle
                .accounts
                .iter()
                .map(|value| DirectHotInstallAccountV5 {
                    key: value.key,
                    account: value.account.clone(),
                    snapshot_for_rollback: value.snapshot_for_rollback,
                })
                .collect(),
            externally_installed_keys: bundle.externally_installed_keys.clone(),
            rollback_snapshot_keys: bundle.rollback_snapshot_keys.clone(),
            market: state.market,
            root: capability.root,
            claims_market: state.claims_market,
            claims_positions: [state.positions[0].0, state.positions[1].0],
            maker_replays: [seller_maker, buyer_maker],
            custody_replay: realm.custody_replay,
            collateral_accounts: product.collateral_accounts,
            custody_routes,
            capability_seal: bundle.artifacts.seal,
            capability_seal_bytes: bundle.artifacts.seal_bytes.clone(),
            descriptor_digest: bundle.artifacts.descriptor.digest,
            claims_caller_authority: Some(claims_caller_authority),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn input() -> DirectHotChainInputV5 {
            DirectHotChainInputV5 {
                registry_program: key(1),
                trading_program: key(2),
                core_program: key(3),
                claims_program: key(4),
                custody_program: key(5),
                rent_program: key(14),
                release_set: [6; 32],
                activation_cache: key(7),
                trading_programdata: key(8),
                core_programdata: key(9),
                claims_programdata: key(10),
                deployment_widths: DirectHotDeploymentWidthsV5::new(1_141_117, 971_053, 934_037)
                    .expect("widths"),
                payer: key(11),
                makers: [key(12), key(13)],
                clock_slot: 50,
                geometry: DirectOrdinaryGeometryV3::CANONICAL,
                trading_semantic_release: [0x33; 32],
                trade: DirectTradeScenarioV1::ZERO_FEE,
            }
        }

        /// The same reproduction gate on the FEE-BEARING trade.
        ///
        /// The builder runs the real host transition engine, so it projects the
        /// two live Custody routes rather than one. A hand fixture the builder
        /// cannot reproduce is a hand fixture that has guessed at the fee leg.
        #[test]
        fn builder_reproduces_the_hand_built_fee_bearing_fixture() {
            let input = DirectHotChainInputV5 {
                trade: DirectTradeScenarioV1::FEE_BEARING,
                ..input()
            };
            let hand = build_direct_hot_chain_fixture_v5(input).expect("hand fee fixture");
            let built =
                build_direct_hot_chain_fixture_via_builder_v1(input).expect("builder fee fixture");
            assert_eq!(built.accounts, hand.accounts);
            assert_eq!(built.custody_routes, hand.custody_routes);
            assert_eq!(built.hot_instruction, hand.hot_instruction);
            assert_eq!(built.signed_messages, hand.signed_messages);
        }

        /// The reproduction gate: the builder's bundle is the hand-built one,
        /// byte for byte.
        #[test]
        fn builder_reproduces_the_hand_built_direct_fixture() {
            let input = input();
            let hand = build_direct_hot_chain_fixture_v5(input).expect("hand fixture");
            let built =
                build_direct_hot_chain_fixture_via_builder_v1(input).expect("builder fixture");
            assert_eq!(built.signed_messages, hand.signed_messages);
            assert_eq!(built.market, hand.market);
            assert_eq!(built.root, hand.root);
            assert_eq!(built.claims_market, hand.claims_market);
            assert_eq!(built.claims_positions, hand.claims_positions);
            assert_eq!(built.maker_replays, hand.maker_replays);
            assert_eq!(built.custody_replay, hand.custody_replay);
            assert_eq!(built.collateral_accounts, hand.collateral_accounts);
            assert_eq!(built.custody_routes, hand.custody_routes);
            assert_eq!(built.capability_seal, hand.capability_seal);
            assert_eq!(built.capability_seal_bytes, hand.capability_seal_bytes);
            assert_eq!(
                built.capability_seal_accounts,
                hand.capability_seal_accounts
            );
            assert_eq!(built.descriptor_digest, hand.descriptor_digest);
            assert_eq!(
                built.hot_instruction.program_id,
                hand.hot_instruction.program_id
            );
            assert_eq!(built.hot_instruction.data, hand.hot_instruction.data);
            assert_eq!(
                built.hot_instruction.accounts.len(),
                hand.hot_instruction.accounts.len()
            );
            for (index, (built_meta, hand_meta)) in built
                .hot_instruction
                .accounts
                .iter()
                .zip(&hand.hot_instruction.accounts)
                .enumerate()
            {
                assert_eq!(built_meta, hand_meta, "instruction meta {index}");
            }
            assert_eq!(built.accounts.len(), hand.accounts.len());
            for (index, (built_account, hand_account)) in
                built.accounts.iter().zip(&hand.accounts).enumerate()
            {
                assert_eq!(built_account, hand_account, "chain account {index}");
            }
            assert_eq!(built.rollback_snapshot_keys, hand.rollback_snapshot_keys);
            let mut built_external = built.externally_installed_keys.clone();
            let mut hand_external = hand.externally_installed_keys.clone();
            built_external.sort_unstable_by_key(Pubkey::to_bytes);
            hand_external.sort_unstable_by_key(Pubkey::to_bytes);
            assert_eq!(built_external, hand_external);
        }
    }
}
