//! Canonical executable Direct Hot account fixture.
//!
//! This module is the sole chain-account constructor for the selected Direct
//! ProgramTest campaign. It derives persisted records and PDAs from public
//! semantic-owner encoders, then packs the logical frame through the selected
//! [`AccountProfileV2`]. The parent Registry harness supplies only release-waist
//! accounts which it already owns.

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CONFIG_STAGING_ACCOUNT_V3,
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
        HOT_TRANSITION_STAGING_ACCOUNT_V3, HotExecutionEnvelopeV3,
    },
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v4::{CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4},
};
use dclutch_claims_svm::{
    CallerRole as ClaimsCallerRole,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
        LiabilityBasisPositionInputV2, encode_liability_basis_market_into_v2,
        encode_liability_basis_position_into_v2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
    sparse_native_transfer_v1::{SparseNativeTransferInputV1, SparseNativeTransferV1},
};
use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_PDA_DOMAIN_V1, CallerRoleV1, CompartmentV1,
    ContextV1, CustodyReplayV1, CustodyRequestV1, DelegatedCustodyRequestV2, OperationV1,
};
use dclutch_direct_codec::{
    execution_v3::{
        DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3, DirectExecutionActionV3, encode_header_v3,
    },
    intent_v2::CompactIntentV2,
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DirectCoordinatesV1, DirectExecutionConfigV1,
        DirectRootStateV1, MakerReplaySeedsV1,
    },
};
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
};
use dclutch_product_payoff_v2_codec::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_product_payoff_v2_codec::runtime_v3::{
    BasisInputV3, BasisKindV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, basis_record_bytes_v3,
    compile_basis_v3, semantic_basis_preimage_v3,
};
use dclutch_product_runtime_v2::{
    ContentId as ProductContentId, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{
    CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1, ExecutionRoleV1,
};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID;
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

use crate::{
    DirectHotArtifactFixtureV5, DirectHotDeploymentWidthsV5, DirectHotFixtureErrorV5,
    build_direct_hot_artifact_fixture_v5, chain::DirectHotInstallAccountV5,
};

const OUTCOME_COUNT: usize = 3;
const OUTCOME_COUNT_U32: u32 = 3;
const GENERATION: u64 = 9;
const PRICE_SCALE: u64 = 100;
const FEE_BPS: u16 = 0;
const FILL: u64 = 10;
const EXECUTION_PRICE: u64 = 50;
const CLAIMS_MARKET_REVISION: u64 = 8;
const SELLER_POSITION_REVISION: u64 = 9;
const BUYER_POSITION_REVISION: u64 = 10;
const CUSTODY_REVISION: u64 = 7;

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
}

/// Complete canonical Direct child instruction and owned account declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectHotChainFixtureV5 {
    /// Trading Hot instruction before Registry wrapping.
    pub hot_instruction: Instruction,
    /// Exact seller and buyer signed intent preimages.
    pub signed_messages: [[u8; 172]; 2],
    /// All accounts, including externally installed release-waist identities.
    pub accounts: Vec<DirectHotInstallAccountV5>,
    /// Accounts already installed by the parent release-waist ProgramTest.
    pub externally_installed_keys: Vec<Pubkey>,
    /// Exact keys expected to change only after every child succeeds.
    pub rollback_snapshot_keys: Vec<Pubkey>,
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

/// Build one canonical executable three-outcome Direct Hot fixture.
pub fn build_direct_hot_chain_fixture_v5(
    input: DirectHotChainInputV5,
) -> Result<DirectHotChainFixtureV5, DirectHotChainFixtureErrorV5> {
    validate_input(input)?;
    let rent = Rent::default();
    let artifacts = build_direct_hot_artifact_fixture_v5(input.deployment_widths)
        .map_err(DirectHotChainFixtureErrorV5::Artifact)?;
    let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
        .encode();
    let product = product_fixture(input, &rent)?;
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
    )?;
    let profile = AccountProfileV2::decode(&artifacts.bundle.account_profile)
        .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
    let runtime = pack_runtime(profile, &mut logical)?;
    let fixed = fixed_hot_accounts(
        input,
        &rent,
        &artifacts,
        &config,
        &product,
        &state,
        &capability,
    )?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).map_err(|_| DirectHotChainFixtureErrorV5::Input)?,
        input.release_set,
        state.market.to_bytes(),
        GENERATION,
        hash(&capability.root_bytes).to_bytes(),
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut data = Vec::with_capacity(HOT_EXECUTION_ENVELOPE_BYTES_V3 + request.len());
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(&request);
    let mut metas = fixed
        .iter()
        .map(|value| value.meta.clone())
        .collect::<Vec<_>>();
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
        root: capability.root,
        claims_market: state.claims_market,
        claims_positions: [state.positions[0].0, state.positions[1].0],
        maker_replays: [capability.seller_maker, capability.buyer_maker],
        custody_replay: realm.custody_replay,
        collateral_accounts: product.collateral_accounts,
    })
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
    core_bytes: Vec<u8>,
    claims_market: Pubkey,
    claims_bytes: Vec<u8>,
    positions: [(Pubkey, Vec<u8>); 2],
}

struct CapabilityFixture {
    root: Pubkey,
    root_bytes: Vec<u8>,
    seller_maker: Pubkey,
    buyer_maker: Pubkey,
    manifest: Vec<u8>,
}

struct CapabilityManifestFixture {
    bytes: Vec<u8>,
    selection: CapabilityExecutionSelectionV1,
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
    _rent: &Rent,
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
        basis_width: u32::try_from(OUTCOME_COUNT)
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        payout_scale: 1,
        knot_denominator: 1,
        knots: &[],
        terms: &[],
        failure_payouts: &[],
    };
    let basis_bytes = basis_record_bytes_v3(BasisKindV3::CategoricalQ1, OUTCOME_COUNT, 0, 0)
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
    let cuts = [0_i128];
    let coefficients = [1_u64, 1, 1];
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
    let rent_credit = rent_credit(input.registry_program, input.payer)?;
    let core_bytes = CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity,
        outstanding_capabilities: 1,
        rent_beneficiary: core_identity(rent_credit.0.to_bytes())?,
        terminal_receipt: None,
    }
    .encode()
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?
    .to_vec();
    let claims_market = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
        &input.claims_program,
    )
    .0;
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
    let mut claims_bytes = vec![0_u8; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + OUTCOME_COUNT * 8];
    encode_liability_basis_market_into_v2(claims_input, &[100, 100, 100], &mut claims_bytes)
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let seller = position(
        input.claims_program,
        claims_market,
        input.makers[0],
        product.semantic_basis,
        SELLER_POSITION_REVISION,
        [100, 0, 0],
    )?;
    let buyer = position(
        input.claims_program,
        claims_market,
        input.makers[1],
        product.semantic_basis,
        BUYER_POSITION_REVISION,
        [0, 0, 0],
    )?;
    let _ = rent;
    Ok(StateFixture {
        market,
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
    balances: [u64; OUTCOME_COUNT],
) -> Result<(Pubkey, Vec<u8>), DirectHotChainFixtureErrorV5> {
    let seeds = ProtocolPositionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let account = Pubkey::find_program_address(&seeds.as_slices(), &claims_program).0;
    let mut bytes = vec![0_u8; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + OUTCOME_COUNT * 8];
    encode_liability_basis_position_into_v2(
        LiabilityBasisPositionInputV2 {
            revision,
            market_account: claims_market.to_bytes(),
            owner: owner.to_bytes(),
            basis_id: basis,
        },
        &balances,
        &mut bytes,
    )
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
            maximum_fill: FILL,
            limit_price: EXECUTION_PRICE,
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
            maximum_fill: FILL,
            limit_price: EXECUTION_PRICE,
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
    put(body, 408, &FILL.to_le_bytes())?;
    put(body, 416, &EXECUTION_PRICE.to_le_bytes())?;
    Ok(output)
}

fn capability_fixture(
    input: DirectHotChainInputV5,
    manifest: CapabilityManifestFixture,
    market: Pubkey,
) -> Result<CapabilityFixture, DirectHotChainFixtureErrorV5> {
    let CapabilityManifestFixture {
        bytes: manifest,
        selection,
    } = manifest;
    let header = CapabilityRootHeaderV1::new(
        core_content(input.release_set)?,
        market.to_bytes(),
        GENERATION,
        selection,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let mut root_bytes = Vec::with_capacity(
        CAPABILITY_ROOT_HEADER_BYTES_V1
            + dclutch_direct_codec::successor::DIRECT_ROOT_STATE_BYTES_V1,
    );
    root_bytes.extend_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&DirectRootStateV1::new().encode());
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &input.trading_program).0;
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
    let descriptor = CapabilityProgramV4::decode(&artifacts.bundle.descriptor)
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
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        capability_id(manifest_digest)?,
        capability_id(descriptor.kind().to_bytes())?,
        capability_id(artifacts.program_set_id)?,
        capability_id(config_digest)?,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    Ok(CapabilityManifestFixture {
        bytes: manifest,
        selection,
    })
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
    let replay = Pubkey::find_program_address(
        &[
            CUSTODY_REPLAY_PDA_DOMAIN_V1,
            &market.to_bytes(),
            &input.release_set,
            &context.to_bytes(),
        ],
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

fn realm_record(
    _collateral: [Pubkey; 3],
) -> Result<[u8; dclutch_realm_contract::REALM_BYTES], DirectHotChainFixtureErrorV5> {
    RealmV1::new(RealmV1Input {
        token_program: LEGACY_TOKEN_PROGRAM_ID,
        collateral_mint: key(0xa4).to_bytes(),
        collateral_adapter_release_id: [0xa5; 32],
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
        claim_count: u32::try_from(OUTCOME_COUNT)
            .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?,
        quantity: FILL,
    })
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)
}

fn custody_request(
    input: DirectHotChainInputV5,
    product: &ProductFixture,
    state: &StateFixture,
    capability: &CapabilityFixture,
    realm: &RealmFixture,
    request: &[u8],
) -> Result<DelegatedCustodyRequestV2, DirectHotChainFixtureErrorV5> {
    let gross = FILL
        .checked_mul(EXECUTION_PRICE)
        .and_then(|value| value.checked_div(PRICE_SCALE))
        .ok_or(DirectHotChainFixtureErrorV5::Encoding)?;
    let parent = hash(request).to_bytes();
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
            destination_owner: input.makers[0].to_bytes(),
            order: parent,
            parent_request_digest: parent,
            order_nonce: 0,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: product.collateral_accounts[0].to_bytes(),
        destination: product.collateral_accounts[1].to_bytes(),
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: realm.mint.to_bytes(),
        token_program: realm.token_program.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: CUSTODY_REVISION,
        resulting_revision: CUSTODY_REVISION + 1,
        amount: gross,
        rent_lamports: 0,
    };
    let delegated = DelegatedCustodyRequestV2 {
        custody,
        starts_atomic_debit: true,
        terminal: true,
        delegate_before: realm.custody_authority.to_bytes(),
        delegate_after: [0; 32],
        total_debit: gross,
        allowance_before: gross,
        allowance_after: 0,
    };
    delegated
        .validate()
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    Ok(delegated)
}

fn fixed_hot_accounts(
    input: DirectHotChainInputV5,
    rent: &Rent,
    artifacts: &DirectHotArtifactFixtureV5,
    config: &[u8],
    product: &ProductFixture,
    state: &StateFixture,
    capability: &CapabilityFixture,
) -> Result<Vec<ChainAccount>, DirectHotChainFixtureErrorV5> {
    let descriptor = CapabilityProgramV4::decode(&artifacts.bundle.descriptor)
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
                dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                capability.manifest.clone(),
            ),
        ),
        (
            HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
            HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
                artifacts.program_set.clone(),
            ),
        ),
        (
            HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
            HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                CAPABILITY_PROGRAM_SCHEMA_ID_V4,
                artifacts.bundle.descriptor.to_vec(),
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
                artifacts.bundle.account_profile.to_vec(),
            ),
        ),
        (
            HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
            HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.request_profile().schema().to_bytes(),
                artifacts.bundle.request_profile.to_vec(),
            ),
        ),
        (
            HOT_TRANSITION_RAW_ACCOUNT_V3,
            HOT_TRANSITION_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.transition().schema().to_bytes(),
                artifacts.bundle.transition.to_vec(),
            ),
        ),
        (
            HOT_EFFECT_RAW_ACCOUNT_V3,
            HOT_EFFECT_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.effect().schema().to_bytes(),
                artifacts.bundle.effect.to_vec(),
            ),
        ),
        (
            HOT_LIFECYCLE_RAW_ACCOUNT_V3,
            HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.lifecycle().schema().to_bytes(),
                artifacts.bundle.lifecycle_policy.to_vec(),
            ),
        ),
        (
            HOT_STRATEGY_RAW_ACCOUNT_V3,
            HOT_STRATEGY_STAGING_ACCOUNT_V3,
            finalized(
                input.registry_program,
                descriptor.strategy().schema().to_bytes(),
                artifacts.bundle.strategy.to_vec(),
            ),
        ),
    ];
    for (raw, staging, record) in finalized_records {
        set(&mut fixed, raw, finalized_raw(rent, &record, false))?;
        set(&mut fixed, staging, vacant(record.staging, false))?;
    }
    set(
        &mut fixed,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        external_data(
            input.activation_cache,
            input.registry_program,
            dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1,
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
    Ok(fixed)
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
) -> Result<Vec<ChainAccount>, DirectHotChainFixtureErrorV5> {
    let mut logical = (0..90)
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
    let fixed = fixed_hot_accounts(input, rent, artifacts, config, product, state, capability)?;
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
        rent_credit_account(rent, input.registry_program, input.makers[0], true)?,
    )?;
    set(&mut logical, 8, vacant(capability.buyer_maker, true))?;
    set(&mut logical, 9, external_payer(input.payer))?;
    set(
        &mut logical,
        10,
        rent_credit_account(rent, input.registry_program, input.makers[1], true)?,
    )?;
    set(&mut logical, 11, program(system_program::ID))?;

    let claims = claims_request(input, product, state, request)?;
    let claims_bytes = claims.to_bytes();
    let claims_packet = hash(&claims_bytes).to_bytes();
    let claims_seeds = CallerAuthoritySeedsV1::new(
        core_content(input.release_set)?,
        state.market.to_bytes(),
        ExecutionRoleV1::Trading,
        hash(request).to_bytes(),
        claims_packet,
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let claims_authority =
        Pubkey::find_program_address(&claims_seeds.as_slices(), &input.trading_program).0;
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
            dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1,
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

    let custody = custody_request(input, product, state, capability, realm, request)?;
    let custody_bytes = custody
        .encode()
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let custody_seeds = CallerAuthoritySeedsV1::new(
        core_content(input.release_set)?,
        state.market.to_bytes(),
        ExecutionRoleV1::Trading,
        hash(request).to_bytes(),
        hash(&custody_bytes).to_bytes(),
    )
    .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let custody_authority =
        Pubkey::find_program_address(&custody_seeds.as_slices(), &input.trading_program).0;
    set(
        &mut logical,
        34,
        external_empty(custody_authority, system_program::ID, false, false),
    )?;
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
        owned(rent, realm.mint, realm.token_program, mint_bytes(), false),
    )?;
    let gross = FILL * EXECUTION_PRICE / PRICE_SCALE;
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
                100,
                Some(realm.custody_authority),
                gross,
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
            token_bytes(realm.mint, input.makers[0], 30, None, 0)?,
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
            token_bytes(realm.mint, input.payer, 40, None, 0)?,
            true,
        ),
    )?;
    // Each child route's `CallerAuthority` is a distinct Trading PDA: its seeds
    // carry that route's own child-request digest, so two routes never share
    // one. Coordinate 48 is the seller-intermediate Custody route's authority
    // and is stated the same way 62 and 76 already are. Copying coordinate 34's
    // authority here made two declared self-representatives one physical
    // account, which `validate_accounts` refuses as `CrossItemAlias`.
    set(
        &mut logical,
        48,
        external_empty(key(0xb0), system_program::ID, false, false),
    )?;
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
    set(
        &mut logical,
        62,
        external_empty(key(0xb1), system_program::ID, false, false),
    )?;
    set(
        &mut logical,
        76,
        external_empty(key(0xb2), system_program::ID, false, false),
    )?;
    Ok(logical)
}

fn pack_runtime(
    profile: AccountProfileV2<'_>,
    logical: &mut [ChainAccount],
) -> Result<Vec<ChainAccount>, DirectHotChainFixtureErrorV5> {
    if logical.len()
        != profile
            .logical_account_count(OUTCOME_COUNT_U32)
            .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?
    {
        return Err(DirectHotChainFixtureErrorV5::Profile);
    }
    let count = profile
        .physical_account_count_with_dynamic_spans(OUTCOME_COUNT_U32, &[])
        .map_err(|_| DirectHotChainFixtureErrorV5::Profile)?;
    let mut packed: Vec<Option<ChainAccount>> = vec![None; count];
    for (coordinate, value) in logical.iter().enumerate() {
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(OUTCOME_COUNT_U32, &[], coordinate)
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
                .physical_account_geometry_with_dynamic_spans(OUTCOME_COUNT_U32, &[], ordinal)
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

fn rent_credit(
    program: Pubkey,
    authority: Pubkey,
) -> Result<(Pubkey, RentCreditV1), DirectHotChainFixtureErrorV5> {
    let authority = RefundAuthority::new(authority.to_bytes())
        .map_err(|_| DirectHotChainFixtureErrorV5::Encoding)?;
    let (key, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, &authority.to_bytes()],
        &program,
    );
    Ok((key, RentCreditV1::new(authority, bump)))
}

fn rent_credit_account(
    rent: &Rent,
    program: Pubkey,
    authority: Pubkey,
    writable: bool,
) -> Result<ChainAccount, DirectHotChainFixtureErrorV5> {
    let (key, credit) = rent_credit(program, authority)?;
    Ok(owned(
        rent,
        key,
        program,
        credit.to_bytes().to_vec(),
        writable,
    ))
}

fn mint_bytes() -> Vec<u8> {
    let mut output = vec![0_u8; SplMint::LEN];
    let value = SplMint {
        mint_authority: COption::None,
        supply: 170,
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
    use super::*;

    fn input() -> DirectHotChainInputV5 {
        DirectHotChainInputV5 {
            registry_program: key(1),
            trading_program: key(2),
            core_program: key(3),
            claims_program: key(4),
            custody_program: key(5),
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
        }
    }

    #[test]
    fn complete_fixture_packs_one_profile13_authority() {
        let input = input();
        let rent = Rent::default();
        let artifacts =
            build_direct_hot_artifact_fixture_v5(input.deployment_widths).expect("artifacts");
        let config = DirectExecutionConfigV1::new(PRICE_SCALE, FEE_BPS, input.payer.to_bytes())
            .expect("config")
            .encode();
        let product = product_fixture(input, &rent).expect("product");
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
            .physical_account_count_with_dynamic_spans(OUTCOME_COUNT_U32, &[])
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
