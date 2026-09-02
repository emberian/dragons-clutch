//! Real-ELF ProgramTest evidence for RationalRepresentationV2 composition.
//!
//! The campaign executes immutable Registry records, Claims economics, real
//! Token-2022 v11 with the selected V2 behavior profile, and canonical Custody.
//! Claim Mints use distinct nonzero/full-domain display decimals while every
//! conservation assertion remains in raw `u64` base units. A test-only SBF
//! caller deliberately refuses after the complete child graph returns to prove
//! transaction-level rollback across every mutable semantic owner.

use std::{env, fs, path::PathBuf, vec::Vec};

mod claim_check;
mod structured_lowering;

use dclutch_claims_sbf::ClaimsSbfError;
use dclutch_claims_sbf::custody_replay_v1::{
    CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1, expected_request_v1,
};
use dclutch_claims_sbf::liability_basis_v2::{
    LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
    encode_liability_basis_market_v2, encode_liability_basis_position_v2,
};
use dclutch_claims_sbf::signed_delta_v3::SignedDeltaSbfErrorV3;
use dclutch_claims_svm::{
    CallerRole,
    custody_replay_v1::ClaimsCustodyReplayRequestV1,
    liability_basis_state_v2::{
        LiabilityBasisMarketLayoutV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionLayoutV2,
    },
    product_basis_terminal_v3::{
        ProductBasisTerminalInputV3, ProductClaimsTerminalAdmissionV3,
        ProductClaimsTerminalInputV3, TERMINAL_CANDIDATE_DOMAIN_V3,
        encode_product_basis_terminal_signed_delta_v3,
        encode_product_claims_terminal_signed_delta_v3,
    },
    protocol_position_v2::{ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionSeedsV2},
    signed_delta_v3::{DeltaDirectionV3, SignedDeltaV3, plan_bytes},
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        TerminalSettlementRequestInputV3, TerminalSettlementRequestV3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1 as CustodyCallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1,
    CustodyVaultSeedsV1, OperationV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
};
use dclutch_market_core_codec::{
    Action, CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase as CorePhase,
    Readiness, Request, StateBumpsV1,
};
use dclutch_operator::{
    Finality, Observation,
    wallet_terminal_payout_v3::{
        WalletTerminalPayoutErrorV3, WalletTerminalPayoutInputV3, WalletTerminalPayoutReportV3,
        WalletTerminalPayoutRouteV3, build_wallet_terminal_payout_v3,
    },
};
use dclutch_product_payoff_v2_codec::{
    price_gate_v1::{
        PRICE_GATE_ATOM_COUNT_OFFSET_V1, PRICE_GATE_DEGREE_OFFSET_V1,
        PRICE_GATE_DENOMINATORS_OFFSET_V1, PRICE_GATE_MAGIC_OFFSET_V1, PRICE_GATE_MAGIC_V1,
        PRICE_GATE_MASS_OFFSET_V1, PRICE_GATE_NUMERATORS_OFFSET_V1, PRICE_GATE_PRICES_OFFSET_V1,
        PRICE_GATE_PROFILE_OFFSET_V1, PRICE_GATE_PROFILE_V1, PRICE_GATE_REQUEST_BYTES_V1,
        PRICE_GATE_SCALE_OFFSET_V1, PRICE_GATE_SCHEMA_VERSION_V1, PRICE_GATE_VERSION_OFFSET_V1,
        PRICE_GATE_WEIGHTS_OFFSET_V1, PRICE_GATE_WIDTH_OFFSET_V1, verify_price_gate_v1,
    },
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product_runtime_v2::{
    ContentId as RuntimeContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V3, AssetV2, CallerRoleV2, RATIONAL_ASSET_ACCOUNT_COUNT_V2,
    RATIONAL_BASE_ACCOUNT_COUNT_V2, RATIONAL_REPLAY_BYTES_V2, RATIONAL_REPLAY_MAGIC_V2,
    RATIONAL_REPLAY_SEED_V2, RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    RATIONAL_SHARD_MINT_SEED_V2, RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
    RATIONAL_TERMINAL_ACCOUNT_COUNT_V2, REQUEST_SELECTED_HEADER_BYTES_V3,
    REQUEST_STRUCTURED_HEADER_BYTES_V3, REQUEST_TERMINAL_HEADER_BYTES_V3, RepresentationActionV2,
    RepresentationRequestHeaderV2, RepresentationRequestV2,
};
use dclutch_rational_representation_v2_kernel::{
    ContentAdmissionV2, DESCRIPTOR_COEFFICIENT_BYTES, DESCRIPTOR_HEADER_BYTES,
    DescriptorAdmissionV2, REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
    product_v3::{
        ProductRepresentationInputV3, ProductRuntimeProjectionV3, RepresentationContextV3,
        TerminalScenarioV3, admit_product_representation_v3,
    },
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1,
    ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_EXPOSURE_SCHEMA_ID_V3, RecordAdmissionV3,
};
use dclutch_resolution_codec::{ResolutionCertificateKindV2, ResolutionCertificateV2};
use dclutch_token_svm::{
    ACCOUNT_BYTES, CollateralAdapterReleaseV1, IMMUTABLE_OWNER_ACCOUNT_BYTES,
    IMMUTABLE_OWNER_ACCOUNT_SUFFIX, PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID,
    TokenAccount,
};
use solana_account::{Account, AccountSharedData};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::{Hash, hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_system_interface::instruction::transfer;
use solana_transaction::{Transaction, versioned::VersionedTransaction};
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;
use spl_token_interface::state::{Account as SplAccount, AccountState, Mint as SplMint};

use dclutch_rational_representation_v2_request_contract::{
    Error as RationalRequestError, generated::ASSET_COEFFICIENT_OFFSET_V3,
};
use dclutch_structured_v2_operator::Error as StructuredOperatorError;
use dclutch_structured_v2_operator::STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2;
use structured_lowering::StructuredBasis;

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe1; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe2; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe3; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe4; 32]);
const TEST_CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe5; 32]);
const RESOLUTION_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe6; 32]);
const TRADING_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xe7; 32]);
const TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
const GENERATION: u64 = 29;
/// The campaign basis width `K`, which is also the Product outcome width `N`.
///
/// Decision 0011 §3b: on this wire `K` IS the full Product outcome width, not
/// the count of backed coordinates — `RepresentationRequestV2::validate`
/// refuses `IssueStructured`/`UnwrapStructured` unless
/// `asset_count == outcome_count`, so every outcome needs an asset row and its
/// materialized account quadruple even at coefficient zero.
const OUTCOME_COUNT: u32 = 3;
/// [`OUTCOME_COUNT`] as an array width.
const K: usize = OUTCOME_COUNT as usize;
const WINNER: u32 = 1;
/// [`WINNER`] as a coordinate index.
const WINNERS: usize = WINNER as usize;
/// The Product's own pre-disclosed failure cell.
///
/// Product Runtime V2 reserves the FINAL result coordinate for explicit
/// failure, and `ResolutionCertificateV2::validate_terminal_product` enforces
/// it from both directions: an ordinary success may not select this coordinate,
/// and a `ResolutionFailure` must select exactly it. So the failure region is
/// not a flag on a terminal -- it is a coordinate, and the holder who exits at
/// failure terms is the holder standing on it.
const FAILURE_SELECTOR: u32 = OUTCOME_COUNT - 1;
/// [`FAILURE_SELECTOR`] as a coordinate index.
const FAILURE_SELECTORS: usize = FAILURE_SELECTOR as usize;
/// Lamports a funded failure walk pays the third party who finishes a market
/// whose own relayer went silent.
///
/// A `ResolutionFailure` certificate whose `work_paid` is zero is refused by
/// `validate_shape`, so the same fact that lets a holder exit at failure terms
/// is the fact that records the walker being paid. The quantity is the one
/// executed against the real Resolution ELF by the relayed campaign
/// (`WALK_BOUNTY_LAMPORTS`, `crates/dclutch-svm-harness/tests/relayed_mainnet_state.rs`).
const FAILURE_WALK_BOUNTY_LAMPORTS: u64 = 250_000;
/// Shard atoms backing one whole native claim.
///
/// Coprime to every coefficient, which is what the campaign basis is FOR: a
/// one-atom backing skew at any single coordinate cannot be presented as a
/// legitimate quantity at another, so `K_i = S * c_i` either holds everywhere
/// or fails visibly at exactly one coordinate.
const DENOMINATOR: u64 = 7;
const RECEIPT_SUPPLY: u64 = 7;
/// Pairwise coprime, and coprime to the denominator.
///
/// This is `dclutch-structured-v2-operator`'s campaign basis
/// (`tests/fixture/mod.rs`), so the host-side derivation and the executing
/// route now measure the same instrument.
const COEFFICIENTS: [u64; K] = [2, 3, 5];
/// The canonical coefficients at the WRONG coordinates.
///
/// A permutation is the sharpest same-width recipe hostile available: it is a
/// canonical composition (the graph encoder refuses a root sharing a factor
/// with its denominator, so a vector like `[4, 6]` over `10` would be rejected
/// as malformed rather than as wrong), it has the same width, and it disagrees
/// at every coordinate.
const PERMUTED_COEFFICIENTS: [u64; K] = [5, 2, 3];
/// One Claims-owned custody Position's claim quantity, per coordinate.
///
/// This and [`ACTOR_CLAIMS`] are the campaign's only free balance parameters.
/// Everything else -- shard supplies, Structured custody balances, the actor's
/// shard balances -- is DERIVED, because `StructuredProjectionV2::validate`
/// (`rational-representation-v2-kernel/src/lib.rs:419-452`) binds all three by
/// identities the chain recomputes on every action:
///
/// | identity | helper |
/// |---|---|
/// | `shard_supply == denominator * native_locked` | [`shard_supply`] |
/// | `structured_custody == receipt_supply * coefficient` | [`structured_shards`] |
/// | `shard_supply == structured_custody + explicit_free` | [`actor_shards`] |
///
/// Writing any of them as a literal is how a basis change silently makes the
/// whole campaign refuse: at the previous `K = 2` basis the literals happened
/// to satisfy all three, and moving the width broke the second one four
/// transactions deep with a bare `0x5008`.
const CUSTODY_CLAIMS: [u64; K] = [4, 7, 8];
/// The actor's own claim quantities: the wallet-held Position's balance vector.
///
/// Coordinate 0 is a LOSING coordinate and the wallet holds one claim there on
/// purpose. A terminal redemption of a losing coordinate is a real transition
/// that pays zero -- the claim is burned, the aggregate's supply falls, and no
/// Custody transfer happens at all -- and it is the only way to reach
/// `terminal_settlement_v3`'s zero-payout branch from a wallet. Nothing else in
/// the campaign reads this coordinate: the aggregate's supply is derived from
/// this vector by [`aggregate_claims`], the shard layer is bound to
/// [`CUSTODY_CLAIMS`] alone, and every representation assertion is at [`WINNER`].
///
/// Coordinate [`FAILURE_SELECTOR`] is the Product's pre-disclosed FAILURE
/// region, and the wallet holds claims there on purpose too. It used to hold
/// zero, which is why no test could ask the question that matters when a market
/// nobody resolved ends on its own terms: does the holder standing in the
/// failure region actually get their collateral back? A wallet with nothing
/// there cannot answer it, and an exit that pays zero is not an exit.
///
/// The quantity is one, not an arbitrary number: the settled coordinate's
/// outstanding supply must be covered by the Hoard, and this vector is what
/// makes `aggregate_claims()` equal [`INITIAL_HOARD_ATOMS`] at BOTH [`WINNER`]
/// and [`FAILURE_SELECTOR`]. Choosing three instead was refused `0x5005`
/// (`ClaimsSbfError::Economic`) with the whole certificate seam already
/// authenticated -- a fully-subscribed coordinate is a protocol fact, not a
/// fixture preference, and the campaign learned it by being told.
const ACTOR_CLAIMS: [u64; K] = [1, 2, 1];
const SHARD_DECIMALS: [u8; K] = [6, u8::MAX, 9];
const RECEIPT_DECIMALS: u8 = 19;
/// The cursor a freshly created Claims-role replay carries.
///
/// This used to be an arbitrary warm value (8) against a replay the fixture
/// PLANTED. The campaign no longer plants one: `create_claims_custody_replay`
/// submits the real `dclutch-claims-sbf` replay-creation route against the real
/// ELFs, Custody creates the account, and the redemption then consumes the
/// cursor that account actually carries. One is what `InitializeReplay` writes,
/// so it is what the redemption must expect.
const CUSTODY_EXPECTED_REVISION: u64 = 1;
const INITIAL_RECIPIENT_ATOMS: u64 = 5;
const INITIAL_HOARD_ATOMS: u64 = 9;
/// Degree-two campaign payout scale.
///
/// At the authenticated coordinate `3/2`, the clamped quadratic Bernstein
/// basis over `[0,0,0,3,3,3]` has exact weights `[1/4,1/2,1/4]`.
/// Cumulative-floor at scale seven pays `[1,4,2]`.
const CURVED_PAYOUT_SCALE: u64 = 7;
const CURVED_RESULT_NUMERATOR: i128 = 3;
const CURVED_RESULT_DENOMINATOR: u64 = 2;
const PACKET_LIMIT: usize = 1_232;
const TOKEN_2022_V11_PROVENANCE: &str = include_str!("../fixtures/token-2022-v11.provenance");

/// The finalized EXPOSURE record's selected identity.
///
/// This is what the request header carries as `graph_id`, what the chain hands
/// `CompositionExposureBundleV3::decode` as `RecordAdmissionV3::selected_id`
/// (`rational_representation_v2.rs:318-327`), and what the descriptor's
/// `graph_id()` accessor returns. Decision 0011 §3d: that accessor's name is
/// the legacy one and it does NOT mean the source composition graph.
const EXPOSURE_ID: [u8; 32] = [0x31; 32];
/// The SOURCE composition DAG's identity — a different record entirely.
///
/// Until this campaign parameterized the descriptor by the derivation, THIS
/// fixture wrote `[0x31; 32]` into both the request header's `graph_id` and the
/// exposure record's own `graph_id` field, so the executing campaign carried
/// exactly the conflation §3d warned Fractional's twin would inherit. The
/// lowering cannot even produce a descriptor while the two are equal:
/// `StructuredTermsV2::require_distinct_identities` puts `shard_exposure` and
/// `graph_id` in one pairwise-distinct set.
const SOURCE_GRAPH_ID: [u8; 32] = [0x32; 32];
/// The composition graph's rank-`K` root node.
const COMPOSITION_ROOT_ID: [u8; 32] = [0x45; 32];
/// The canonical translation record the composition descriptor names.
const CANONICAL_TRANSLATION_ID: [u8; 32] = [0x46; 32];
/// Selected token-behavior profile for the shard layer.
const SHARD_TOKEN_BEHAVIOR_ID: [u8; 32] = [0x47; 32];
/// Selected token-behavior profile for the receipt layer.
const RECEIPT_TOKEN_BEHAVIOR_ID: [u8; 32] = [0x48; 32];

/// The caller-chosen founding action context one `DCLTGMF1` founding carries.
///
/// `GenericFoundingRequestV1::context` is caller-owned: the protocol never
/// requires it to be the Market address, and the campaign that founded the
/// first open Market derives it from the Market, generation, and release set
/// under its own domain (`tools/local-validator/bootstrap/successor/src/market.rs`).
/// Any 32 bytes are admissible, so this fixture uses opaque ones on purpose —
/// a fixture that picked a *derivable* context would silently re-admit the
/// assumption this campaign exists to refuse.
const FOUNDING_ACTION_CONTEXT_V1: [u8; 32] = [0x6f; 32];

/// The Market's Custody namespace, exactly as the founding routes derive it.
///
/// `generic_market_founding_v1::authenticate_projected_lock_join_v1` pins
/// `context_digest = SHA-256(PROJECTED_HOARD_CONTEXT_DOMAIN_V1 || context)`,
/// Custody's `open_hoard` creates the Hoard Vault under that digest, and
/// `RealizeAndClose` rewrites the projection in place as the Market's normal
/// replay at the same digest. This is therefore the one context coordinate the
/// founded Market's collateral actually lives under — not the Market address.
fn founding_custody_context() -> [u8; 32] {
    hashv(&[
        PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
        &FOUNDING_ACTION_CONTEXT_V1,
    ])
    .to_bytes()
}

/// The shard Mint supply of one coordinate.
///
/// Not a constant. It is `CUSTODY_CLAIMS[i] * DENOMINATOR` by definition —
/// shards ARE claims divided by the denominator — and restating it as a literal
/// is how a basis change quietly makes every conservation assertion vacuous.
fn shard_supply(index: usize) -> u64 {
    CUSTODY_CLAIMS
        .get(index)
        .copied()
        .expect("custody claims")
        .checked_mul(DENOMINATOR)
        .expect("shard supply")
}

/// Structured custody's shard balance for one coordinate.
///
/// `structured_custody == receipt_supply * coefficient` is a projection
/// invariant, not a fixture choice: every outstanding receipt atom is backed by
/// exactly `c_i` shard atoms held in custody.
fn structured_shards() -> [u64; K] {
    std::array::from_fn(|index| {
        RECEIPT_SUPPLY
            .checked_mul(COEFFICIENTS.get(index).copied().expect("coefficient"))
            .expect("structured custody backing")
    })
}

/// The actor's shard balance for one coordinate: the explicit free remainder.
fn actor_shards() -> [u64; K] {
    std::array::from_fn(|index| {
        shard_supply(index)
            .checked_sub(
                structured_shards()
                    .get(index)
                    .copied()
                    .expect("structured custody"),
            )
            .expect("the shard supply must cover its own custody backing")
    })
}

/// Per-outcome claim totals the `LiabilityBasisV2` aggregate carries.
fn aggregate_claims() -> [u64; K] {
    std::array::from_fn(|index| {
        CUSTODY_CLAIMS
            .get(index)
            .copied()
            .expect("custody claims")
            .checked_add(ACTOR_CLAIMS.get(index).copied().expect("actor claims"))
            .expect("aggregate claims")
    })
}

/// The actor's shard balances after one `IssueStructured` of quantity one.
///
/// `K_i = S * c_i` (`plan.rs:263`): issuing one receipt atom moves exactly
/// `c_i` shard atoms of coordinate `i` from the actor to Structured custody.
fn actor_shards_after_issue() -> [u64; K] {
    std::array::from_fn(|index| {
        actor_shards()
            .get(index)
            .copied()
            .expect("actor shards")
            .checked_sub(COEFFICIENTS.get(index).copied().expect("coefficient"))
            .expect("issue leaves the actor solvent")
    })
}

/// Structured custody's shard balances after that same issue.
fn structured_shards_after_issue() -> [u64; K] {
    std::array::from_fn(|index| {
        structured_shards()
            .get(index)
            .copied()
            .expect("structured shards")
            .checked_add(COEFFICIENTS.get(index).copied().expect("coefficient"))
            .expect("structured shards")
    })
}

/// The actor's shard balances after one `Denominate` of one whole claim.
fn actor_shards_after_denominate() -> [u64; K] {
    std::array::from_fn(|index| {
        let balance = actor_shards().get(index).copied().expect("actor shards");
        if index == usize::try_from(WINNER).expect("winner index") {
            balance
                .checked_add(DENOMINATOR)
                .expect("denominated shards")
        } else {
            balance
        }
    })
}

/// One Claims custody owner's Position: claims at its own coordinate only.
fn custody_claims(index: usize) -> [u64; K] {
    std::array::from_fn(|slot| {
        if slot == index {
            CUSTODY_CLAIMS.get(index).copied().expect("custody claims")
        } else {
            0
        }
    })
}

struct Artifacts {
    claims: Vec<u8>,
    custody: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    resolution: Vec<u8>,
    token_2022: Vec<u8>,
    caller: Vec<u8>,
}

#[derive(Clone, Copy)]
struct AssetFixture {
    custody_owner: Pubkey,
    position: Pubkey,
    mint: Pubkey,
    actor_token: Pubkey,
    structured_token: Pubkey,
    obsolete_structured_ata: Pubkey,
}

#[derive(Clone, Copy)]
struct TerminalFixture {
    certificate: Pubkey,
    realm_raw: Pubkey,
    realm_staging: Pubkey,
    custody_caller: Pubkey,
    custody_replay: Pubkey,
    collateral_mint: Pubkey,
    hoard: Pubkey,
    recipient: Pubkey,
    custody_authority: Pubkey,
}

struct Fixture {
    actor: Keypair,
    basis_profile: ProductBasisProfileV1,
    /// The terminal interpretation authenticated by this fixture's
    /// certificate. Builders consume this fact rather than reconstructing it
    /// from a profile name.
    terminal_scenario: TerminalScenarioV3,
    /// Exact Hoard principal required by this terminal partition and the
    /// aggregate's outstanding supplies.
    initial_hoard_atoms: u64,
    /// The coordinate this fixture's terminal committed to.
    ///
    /// [`WINNER`] under a provider-backed resolution, [`FAILURE_SELECTOR`] when
    /// the Market ended on its own pre-disclosed failure terms. Every wallet
    /// payout defaults its claim coordinate, its quantity and its host-side
    /// Product evaluation to this, so a failure fixture settles at the failure
    /// region without any test restating which coordinate that is.
    terminal_winner: u32,
    release_set: [u8; 32],
    realm_id: [u8; 32],
    parent_context: [u8; 32],
    /// The Market's Custody namespace — see [`founding_custody_context`].
    ///
    /// Deliberately NOT the representation request's `parent_context` and
    /// deliberately NOT the Market address. Those three were one value here
    /// until this campaign separated them, which is why a Hoard that does not
    /// live at the Market address had never been exercised.
    custody_context: [u8; 32],
    market: Pubkey,
    aggregate: Pubkey,
    actor_position: Pubkey,
    activation_cache: Pubkey,
    claims_programdata: Pubkey,
    custody_programdata: Pubkey,
    core_programdata: Pubkey,
    resolution_programdata: Pubkey,
    caller_programdata: Pubkey,
    representation_authority: Pubkey,
    descriptor_id: [u8; 32],
    descriptor_raw: Pubkey,
    descriptor_staging: Pubkey,
    alternate_descriptor_raw: Pubkey,
    alternate_descriptor_staging: Pubkey,
    graph_id: [u8; 32],
    graph_raw: Pubkey,
    graph_staging: Pubkey,
    alternate_graph_raw: Pubkey,
    alternate_graph_staging: Pubkey,
    linked_basis_record: Pubkey,
    linked_basis_staging: Pubkey,
    product_record: Pubkey,
    product_staging: Pubkey,
    result_domain_record: Pubkey,
    result_domain_staging: Pubkey,
    portfolio_record: Pubkey,
    portfolio_staging: Pubkey,
    /// Finalized Product graph-root digest.
    ///
    /// These five are what the family-neutral terminal-settlement wire needs and
    /// the Rational wire does not: `DCLTSQ03` names the Product, basis and
    /// exposure identities directly, where `RepresentationRequestV2` reaches
    /// them through the descriptor. Same records, same fixture, different wire.
    product_digest: [u8; 32],
    /// Product-owned semantic LiabilityBasisV2 identity.
    semantic_basis_id: [u8; 32],
    /// Finalized ProductBasisV3 raw-record digest.
    linked_basis_digest: [u8; 32],
    /// SHA-256 of the finalized composition-exposure bytes.
    graph_digest: [u8; 32],
    /// The Core terminal receipt certificate account, when this fixture is resolved.
    ///
    /// The field name follows the stable V3 request member; the bytes are an
    /// account identity, never a content digest.
    terminal_record_digest: Option<[u8; 32]>,
    /// Finalized ResultDomainV2 record digest.
    result_domain_digest: [u8; 32],
    /// Exact finalized ProductBasisV3 bytes.
    linked_basis_bytes: Vec<u8>,
    /// Exact finalized composition-exposure bytes.
    graph_bytes: Vec<u8>,
    representation_replay: Pubkey,
    receipt_mint: Pubkey,
    actor_receipt: Pubkey,
    assets: [AssetFixture; K],
    /// The Structured basis this Market's descriptor was DERIVED from.
    basis: StructuredBasis,
    terminal_accounts: Option<TerminalFixture>,
    /// Current five-action Structured release and activated child root for the
    /// real common-Hot campaign. Ordinary direct-Claims fixtures leave both
    /// absent so their historical release/caller bytes remain unchanged.
    hot_release: Option<common_hot_open::HotReleaseFixture>,
    capability_root: Option<Pubkey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    replay: Account,
    aggregate: Account,
    actor_position: Account,
    positions: [Account; K],
    receipt_mint: Account,
    actor_receipt: Account,
    shard_mints: [Account; K],
    actor_shards: [Account; K],
    structured_shards: [Account; K],
    obsolete_structured_shards: [Account; K],
    custody_replay: Option<Account>,
    hoard: Option<Account>,
    recipient: Option<Account>,
}

struct Submission {
    accepted: bool,
    compute_units: u64,
    wire_bytes: usize,
    logs: Vec<String>,
}

mod common_hot_open {
    use super::*;
    use dclutch_account_profile_contract::v2::AccountProfileV2;
    use dclutch_bearer_v2_operator::{
        CheckedRationalHotOuterReleaseV3, ConstructedHotOpenSelectedV3,
        ConstructedHotOpenStructuredV3, ConstructedHotTerminalV3,
        RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3, RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3,
        RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3, RationalOpenCapabilityProgramSetInputV6,
        RationalOpenCapabilityProgramSetV3, RationalOpenSelectedBundleInputV6,
        RationalOpenSelectedHotBundleV3, RationalOpenSelectedHotStateV3,
        RationalOpenStructuredHotBundleV3, RationalOpenStructuredHotStateV3,
        RationalOpenStructuredSelectedBundleInputV6, RationalTerminalAccountProfileInputV3,
        RationalTerminalHotBundleV3, RationalTerminalSelectedBundleInputV6,
        build_rational_open_capability_program_set_v6, build_rational_open_selected_bundle_v6,
        build_rational_open_selected_hot_instruction_v3,
        build_rational_open_structured_hot_instruction_v3,
        build_rational_open_structured_selected_bundle_v6,
        build_rational_terminal_hot_instruction_v3, build_rational_terminal_selected_bundle_v6,
        construct_chain_hot_denominate_v3, construct_chain_hot_issue_structured_v3,
        construct_chain_hot_redeem_terminal_v3, encode_open_capability_lifecycle_policy_v5,
    };
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
        FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
        funding::{CompartmentFundingV1, FundingAmountsV1},
    };
    use dclutch_capability_program_contract::{
        SelectedRecordBumpsV1, set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        v4::CapabilityProgramV4,
    };
    use dclutch_chain_bundle_builder::{
        WaistFactsV1,
        artifacts::{ArtifactSetV1, derive_record},
        bundle::{BuiltBundleV1, BundleInputV1, FixedCorpusV1, ScenarioV1, build_bundle},
        frame::BuiltAccountV1,
    };
    use dclutch_rational_representation_v2_contract::{
        RepresentationCoordinateV2, TokenBehaviorRecordAdmissionV2, authenticate_token_behavior_v2,
    };
    use dclutch_rational_representation_v2_operator::{
        AssetObservationV2, FinalizedRecordObservationV2, ObservedAccountV2,
        ProductEvidenceObservationV2, RationalObservationV2, ReplayObservationV2,
        SelectedActionInputV2, StructuredActionInputV2, TerminalObservationV2,
    };
    use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
    use dclutch_structured_v2_kernel::{
        STRUCTURED_CAPABILITY_KIND_ID_V2, STRUCTURED_CAPACITY_PROFILE_ID_V2,
        STRUCTURED_ROOT_BYTES_V2, STRUCTURED_ROOT_SCHEMA_ID_V2,
    };
    use dclutch_token_svm::{TOKEN_BEHAVIOR_SELECTION_BYTES_V2, TokenBehaviorSelectionV2};

    pub(super) struct ManifestSelection {
        pub(super) bytes: Vec<u8>,
        pub(super) selection: CapabilityExecutionSelectionV1,
        pub(super) record_bumps: SelectedRecordBumpsV1,
    }

    pub(super) struct HotReleaseFixture {
        pub(super) denominate: RationalOpenSelectedHotBundleV3,
        pub(super) issue: RationalOpenStructuredHotBundleV3,
        pub(super) redeem: RationalTerminalHotBundleV3,
        pub(super) set: RationalOpenCapabilityProgramSetV3,
        pub(super) manifest: ManifestSelection,
    }

    /// Both bumps derive through `RecordKeyV1`, the constructor the Record
    /// contract exports for exactly this, rather than respelling the seed
    /// tuple here. A test that spells the tuple becomes a second author for
    /// the address, and the seam register's own rule is that a NEW file
    /// restating an existing domain is corrected rather than filed beside the
    /// existing debt.
    fn record_bumps(schema: [u8; 32], digest: [u8; 32]) -> (u8, u8) {
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(schema).expect("schema release id"),
            ContentDigest::new(digest).expect("content digest"),
        );
        let bump = |seeds: RecordPdaSeedsV1| {
            Pubkey::find_program_address(
                &[
                    seeds.domain(),
                    seeds.schema_release_id().as_bytes(),
                    seeds.expected_digest().as_bytes(),
                ],
                &REGISTRY_PROGRAM_ID,
            )
            .1
        };
        (
            bump(key.raw_record_pda_seeds()),
            bump(key.staging_cursor_pda_seeds()),
        )
    }

    fn manifest(
        set: &RationalOpenCapabilityProgramSetV3,
        descriptor_bytes: &[u8],
    ) -> ManifestSelection {
        let descriptor = CapabilityProgramV4::decode(descriptor_bytes).expect("descriptor");
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(1).expect("native activation funding"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("funding compartments");
        let entry = CapabilityEntryV1::new(
            dclutch_capability_contract::ContentId::new(descriptor.kind().to_bytes())
                .expect("kind"),
            dclutch_capability_contract::ContentId::new(set.program_set_id).expect("ProgramSet"),
            dclutch_capability_contract::ContentId::new(set.token_behavior_selection_id)
                .expect("config"),
            dclutch_capability_contract::ContentId::new(descriptor.capacity_profile().to_bytes())
                .expect("capacity"),
            dclutch_capability_contract::ContentId::new(descriptor.root_schema().to_bytes())
                .expect("root schema"),
            dclutch_capability_contract::ContentId::new(descriptor.derivation_policy().to_bytes())
                .expect("lifecycle"),
            ActivationPolicy::PrepaidLazy,
            100,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(amounts, None).expect("funding quote"),
        )
        .expect("manifest entry");
        let mut bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut bytes).expect("manifest");
        let manifest_digest = hash(&bytes).to_bytes();
        let manifest_bumps = record_bumps(
            dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            manifest_digest,
        );
        let config_bumps = record_bumps(
            descriptor.config_schema().to_bytes(),
            set.token_behavior_selection_id,
        );
        let program_set_bumps = record_bumps(
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            set.program_set_id,
        );
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            ContentId::new(manifest_digest).expect("manifest"),
            ContentId::new(descriptor.kind().to_bytes()).expect("kind"),
            ContentId::new(set.program_set_id).expect("ProgramSet"),
            ContentId::new(set.token_behavior_selection_id).expect("config"),
        )
        .expect("selection")
        .with_capability_release_record_bumps(program_set_bumps.0, program_set_bumps.1);
        ManifestSelection {
            bytes,
            selection,
            record_bumps: SelectedRecordBumpsV1::new(
                manifest_bumps.0,
                manifest_bumps.1,
                config_bumps.0,
                config_bumps.1,
            ),
        }
    }

    pub(super) fn compile_release(
        realm: [u8; 32],
        release_set: [u8; 32],
        product_basis: &[u8],
    ) -> HotReleaseFixture {
        let selection = TokenBehaviorSelectionV2::new(realm, release_set).expect("selection");
        let lifecycle = encode_open_capability_lifecycle_policy_v5().expect("empty lifecycle");
        let basis_width = u32::try_from(product_basis.len()).expect("basis bytes");
        let mut selected_lengths = [0_u32; RATIONAL_OPEN_SELECTED_LOGICAL_ACCOUNTS_V3 as usize];
        selected_lengths[4] = basis_width;
        selected_lengths[29] = basis_width;
        // TEN MORE COORDINATES, and the reason they were missing is that this
        // profile had never been projected. The comment below on
        // `structured_lengths` says the account-projection kernel refuses
        // `DataLengthMismatch` on every coordinate left at zero, and that lane
        // calibrated the STRUCTURED profile and stopped -- correctly, because
        // the selected bundle could not be built at all until the Bearer gate
        // learned to step aside for a fractional descriptor. The first
        // `plan_denominate` that returned a bundle refused here on all ten at
        // once.
        //
        // Each is taken from the constant or byte function that owns it, never
        // from the account it will be compared against: a fixture that reads a
        // width off the observed account and then declares it has checked
        // nothing. Logical coordinate is the Claims child index plus five.
        // The three INJECTED Hot-evidence coordinates, which are not part of the
        // Claims child frame at all and so are invisible to any check that
        // walks it: the capability root, the Product record and the Portfolio
        // record.
        selected_lengths[0] = u32::try_from(
            dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
                + dclutch_structured_v2_kernel::STRUCTURED_ROOT_BYTES_V2,
        )
        .expect("capability root width");
        selected_lengths[2] = u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("Product record width");
        selected_lengths[3] = u32::try_from(
            dclutch_product_runtime_v2::portfolio_record_bytes(COEFFICIENTS.len())
                .expect("Portfolio record width"),
        )
        .expect("Portfolio record width");
        selected_lengths[10] = u32::try_from(
            dclutch_rational_representation_v2_kernel::descriptor_v3::representation_descriptor_bytes_v3(K)
                .expect("Rational descriptor width"),
        )
        .expect("Rational descriptor width");
        selected_lengths[12] = u32::try_from(
            dclutch_representation_composition_v3_kernel::composition_exposure_bytes_v3(
                OUTCOME_COUNT,
                OUTCOME_COUNT,
            )
            .expect("composition exposure width"),
        )
        .expect("composition exposure width");
        selected_lengths[14] =
            u32::try_from(<Rent as solana_sdk::sysvar::SysvarSerialize>::size_of())
                .expect("Rent sysvar width");
        selected_lengths[16] =
            u32::try_from(RATIONAL_REPLAY_BYTES_V2).expect("representation replay width");
        selected_lengths[17] = u32::try_from(
            dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_MARKET_HEADER_BYTES_V2
                + K * 8,
        )
        .expect("Claims aggregate width");
        selected_lengths[18] =
            u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("activation width");
        selected_lengths[22] =
            u32::try_from(dclutch_market_core_codec::STATE_BYTES).expect("Core market width");
        // The actor's Position and the selected coordinate's custody Position
        // are the same shape; the selected actions are the only ones that carry
        // a live actor Position at all, which is why neither appears above.
        let position_width = u32::try_from(
            dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_POSITION_HEADER_BYTES_V2
                + K * 8,
        )
        .expect("Claims Position width");
        selected_lengths[28] = position_width;
        selected_lengths[37] = position_width;
        selected_lengths[33] = u32::try_from(
            dclutch_product_runtime_v2::result_domain_record_bytes(
                K.checked_sub(2).expect("K >= 2"),
            )
            .expect("ResultDomain record width"),
        )
        .expect("ResultDomain record width");
        let mut structured_lengths = [0_u32; RATIONAL_OPEN_STRUCTURED_FIXED_ACCOUNTS_V3 as usize];
        structured_lengths[4] = basis_width;
        structured_lengths[29] = basis_width;
        // The widths that ARE knowable when a release is authored, each taken
        // from the constant that owns it rather than typed.
        structured_lengths[0] = u32::try_from(
            dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
                + dclutch_structured_v2_kernel::STRUCTURED_ROOT_BYTES_V2,
        )
        .expect("capability root width");
        structured_lengths[17] = u32::try_from(
            dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_MARKET_HEADER_BYTES_V2
                + K * 8,
        )
        .expect("Claims aggregate width");
        structured_lengths[18] =
            u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("activation width");
        structured_lengths[22] =
            u32::try_from(dclutch_market_core_codec::STATE_BYTES).expect("Core market width");
        // The five finalized-record coordinates and the Rent sysvar, each from
        // the constant or the byte function that owns it. `d4cd3b27` declared
        // the four widths a release can know off its own artifacts and stopped
        // there; the account-projection kernel refuses `DataLengthMismatch` on
        // every coordinate still left at zero, which is what kept the physical
        // Trading common-Hot campaign from reaching submission. Logical
        // coordinates are the child's own index plus five (`build_hot`), so
        // these are `rational_representation_v2.rs`'s PRODUCT_RECORD,
        // PORTFOLIO_RECORD, DESCRIPTOR_RAW, GRAPH_RAW, RENT_SYSVAR and
        // RESULT_DOMAIN_RECORD in that order.
        structured_lengths[2] =
            u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("Product record width");
        structured_lengths[3] = u32::try_from(
            dclutch_product_runtime_v2::portfolio_record_bytes(COEFFICIENTS.len())
                .expect("Portfolio record width"),
        )
        .expect("Portfolio record width");
        structured_lengths[10] = u32::try_from(
            dclutch_rational_representation_v2_kernel::descriptor_v3::representation_descriptor_bytes_v3(K)
                .expect("Rational descriptor width"),
        )
        .expect("Rational descriptor width");
        structured_lengths[12] = u32::try_from(
            dclutch_representation_composition_v3_kernel::composition_exposure_bytes_v3(
                OUTCOME_COUNT,
                OUTCOME_COUNT,
            )
            .expect("composition exposure width"),
        )
        .expect("composition exposure width");
        structured_lengths[14] =
            u32::try_from(<Rent as solana_sdk::sysvar::SysvarSerialize>::size_of())
                .expect("Rent sysvar width");
        structured_lengths[33] = u32::try_from(
            dclutch_product_runtime_v2::result_domain_record_bytes(
                K.checked_sub(2).expect("K >= 2"),
            )
            .expect("ResultDomain record width"),
        )
        .expect("ResultDomain record width");
        let mut terminal_lengths = [0_u32; RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize];
        terminal_lengths[1] =
            u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("selection bytes");
        terminal_lengths[4] = basis_width;
        terminal_lengths[29] = basis_width;
        // The same eleven the selected profile needed, for the same reason and
        // from the same owners: this profile had never been projected either.
        // Three of them are INJECTED Hot-evidence coordinates outside the Claims
        // child frame, which is why a check that walks the child cannot see them.
        terminal_lengths[0] = selected_lengths[0];
        terminal_lengths[2] = selected_lengths[2];
        terminal_lengths[3] = selected_lengths[3];
        terminal_lengths[10] = selected_lengths[10];
        terminal_lengths[12] = selected_lengths[12];
        terminal_lengths[14] = selected_lengths[14];
        terminal_lengths[17] = selected_lengths[17];
        terminal_lengths[18] = selected_lengths[18];
        terminal_lengths[22] = selected_lengths[22];
        terminal_lengths[33] = selected_lengths[33];
        terminal_lengths[37] = selected_lengths[37];
        let selected = |action| {
            build_rational_open_selected_bundle_v6(RationalOpenSelectedBundleInputV6 {
                action,
                logical_data_lengths: &selected_lengths,
                product_basis,
                kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
                token_behavior_selection: selection,
                root_schema: STRUCTURED_ROOT_SCHEMA_ID_V2,
                lifecycle_policy: &lifecycle,
                capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
                root_state_bytes: u32::try_from(STRUCTURED_ROOT_BYTES_V2)
                    .expect("Structured root width"),
            })
            .expect("selected artifacts")
        };
        let structured = |action| {
            build_rational_open_structured_selected_bundle_v6(
                RationalOpenStructuredSelectedBundleInputV6 {
                    action,
                    fixed_data_lengths: &structured_lengths,
                    item_data_lengths: [
                        u32::try_from(
                            dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_POSITION_HEADER_BYTES_V2
                                + K * 8,
                        )
                        .expect("custody Position width"),
                        0,
                        0,
                        0,
                    ],
                    product_basis,
                    representation_outcome_count: OUTCOME_COUNT,
                    token_behavior_selection: selection,
                    kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
                    root_schema: STRUCTURED_ROOT_SCHEMA_ID_V2,
                    lifecycle_policy: &lifecycle,
                    capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
                    root_state_bytes: u32::try_from(STRUCTURED_ROOT_BYTES_V2)
                        .expect("Structured root width"),
                },
            )
            .expect("Structured artifacts")
        };
        let denominate = selected(RepresentationActionV2::Denominate);
        let reconstitute = selected(RepresentationActionV2::Reconstitute);
        let issue = structured(RepresentationActionV2::IssueStructured);
        let unwrap = structured(RepresentationActionV2::UnwrapStructured);
        let redeem =
            build_rational_terminal_selected_bundle_v6(RationalTerminalSelectedBundleInputV6 {
                account_profile: RationalTerminalAccountProfileInputV3 {
                    logical_data_lengths: &terminal_lengths,
                    product_basis,
                },
                kind: STRUCTURED_CAPABILITY_KIND_ID_V2,
                token_behavior_selection: selection,
                root_schema: STRUCTURED_ROOT_SCHEMA_ID_V2,
                lifecycle_policy: &lifecycle,
                capacity_profile: STRUCTURED_CAPACITY_PROFILE_ID_V2,
                root_state_bytes: u32::try_from(STRUCTURED_ROOT_BYTES_V2)
                    .expect("Structured root width"),
            })
            .expect("terminal artifacts");
        let set = build_rational_open_capability_program_set_v6(
            RationalOpenCapabilityProgramSetInputV6 {
                token_behavior_selection: selection,
                denominate: &denominate,
                reconstitute: &reconstitute,
                issue_structured: &issue,
                unwrap_structured: &unwrap,
                redeem_terminal: &redeem,
            },
        )
        .expect("five-action Structured release");
        let manifest = manifest(&set, &issue.descriptor);
        HotReleaseFixture {
            denominate,
            issue,
            redeem,
            set,
            manifest,
        }
    }

    fn observed<'a>(key: Pubkey, account: &'a Account) -> ObservedAccountV2<'a> {
        ObservedAccountV2 {
            key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: &account.data,
        }
    }

    fn record<'a>(
        schema_id: [u8; 32],
        raw_key: Pubkey,
        raw: &'a Account,
        staging_key: Pubkey,
        staging: &'a Account,
    ) -> FinalizedRecordObservationV2<'a> {
        FinalizedRecordObservationV2 {
            schema_id,
            raw: observed(raw_key, raw),
            staging: observed(staging_key, staging),
        }
    }

    async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
        // NAMES THE KEY. An `expect` that says only "observed account" turns a
        // located defect into a search across a forty-nine coordinate frame.
        context
            .banks_client
            .get_account(key)
            .await
            .expect("account query")
            .unwrap_or_else(|| panic!("no account at {key}"))
    }

    /// The semantic role the Claims child frame carries at `child_index`.
    ///
    /// Read off the request contract that specifies the frame, so this test can
    /// name a coordinate instead of a base58 address, and can grant a vacancy
    /// to the coordinates that EARN one rather than to whatever happens to be
    /// absent.
    pub(super) fn frame_role(
        action: RepresentationActionV2,
        child_index: usize,
    ) -> Option<RepresentationCoordinateV2> {
        let assets = if matches!(
            action,
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
        ) {
            usize::try_from(OUTCOME_COUNT).expect("outcome count")
        } else {
            1
        };
        dclutch_rational_representation_v2_contract::REPRESENTATION_FRAME_SPEC_V2.coordinate(
            child_index,
            assets,
            action == RepresentationActionV2::RedeemTerminal,
        )
    }

    /// The same observation, naming the ROLE the missing coordinate carries.
    ///
    /// `account` names the address, which locates nothing on its own: a frame
    /// coordinate's address is derived, so a reader who sees only the base58
    /// has to walk the whole child frame to find out what was supposed to be
    /// there. The request contract knows the role at every index.
    async fn named_account(
        context: &mut ProgramTestContext,
        key: Pubkey,
        action: RepresentationActionV2,
        child_index: usize,
    ) -> Account {
        let role = frame_role(action, child_index);
        context
            .banks_client
            .get_account(key)
            .await
            .expect("account query")
            .unwrap_or_else(|| {
                panic!("no account at {key} for child coordinate {child_index} ({role:?})")
            })
    }

    /// Observe one coordinate that is ALLOWED to hold nothing.
    ///
    /// A finalized record's staging cursor is absent by definition, and an
    /// address holding nothing is exactly what the runtime materialises for a
    /// frame slot: zero lamports, System-owned, empty. Reading it as `vacant`
    /// is observing the chain rather than restating it -- and it is the reason
    /// this helper is separate from `account`, which must still refuse for a
    /// coordinate that is required to exist.
    async fn vacant_or_account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
        context
            .banks_client
            .get_account(key)
            .await
            .expect("account query")
            .unwrap_or(Account {
                lamports: 0,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            })
    }

    async fn plan(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
        action: RepresentationActionV2,
    ) -> HotPlanV3 {
        let terminal = action == RepresentationActionV2::RedeemTerminal;
        let activation = account(context, fixture.activation_cache).await;
        let descriptor_raw = account(context, fixture.descriptor_raw).await;
        let descriptor_staging = vacant_or_account(context, fixture.descriptor_staging).await;
        let graph_raw = account(context, fixture.graph_raw).await;
        let graph_staging = vacant_or_account(context, fixture.graph_staging).await;
        let linked_raw = account(context, fixture.linked_basis_record).await;
        let linked_staging = vacant_or_account(context, fixture.linked_basis_staging).await;
        let product_raw = account(context, fixture.product_record).await;
        let product_staging = vacant_or_account(context, fixture.product_staging).await;
        let domain_raw = account(context, fixture.result_domain_record).await;
        let domain_staging = vacant_or_account(context, fixture.result_domain_staging).await;
        let portfolio_raw = account(context, fixture.portfolio_record).await;
        let portfolio_staging = vacant_or_account(context, fixture.portfolio_staging).await;
        let market = account(context, fixture.market).await;
        let aggregate = account(context, fixture.aggregate).await;
        let replay = account(context, fixture.representation_replay).await;
        let receipt_mint = account(context, fixture.receipt_mint).await;
        // A terminal redemption references NEITHER of these -- see the
        // observation below -- and a terminal fixture need not have created
        // them, so they are observed rather than required on that path only.
        let actor_receipt = if terminal {
            vacant_or_account(context, fixture.actor_receipt).await
        } else {
            account(context, fixture.actor_receipt).await
        };
        let actor_position = if terminal {
            vacant_or_account(context, fixture.actor_position).await
        } else {
            account(context, fixture.actor_position).await
        };

        let terminal_fixture = fixture.terminal_accounts;
        let terminal_observed = match terminal_fixture {
            Some(accounts) if terminal => Some((
                account(context, accounts.realm_raw).await,
                vacant_or_account(context, accounts.realm_staging).await,
                account(context, accounts.certificate).await,
                // A LOSING coordinate pays zero and needs no replay at all --
                // `construct_redeem_terminal` only authenticates one when the
                // payout is positive -- so absence here is a real state and the
                // runtime materialises exactly this for it.
                vacant_or_account(context, accounts.custody_replay).await,
                account(context, accounts.collateral_mint).await,
                account(context, accounts.hoard).await,
                account(context, accounts.recipient).await,
            )),
            _ => None,
        };
        let selected = action == RepresentationActionV2::Denominate || terminal;
        let selected_index = usize::try_from(WINNER).expect("selected outcome");
        let asset_range: Vec<usize> = if selected {
            vec![selected_index]
        } else {
            (0..K).collect()
        };
        let mut asset_accounts = Vec::with_capacity(asset_range.len());
        for index in &asset_range {
            let fixture_asset = fixture.assets.get(*index).expect("asset");
            asset_accounts.push((
                account(context, fixture_asset.position).await,
                account(context, fixture_asset.mint).await,
                account(context, fixture_asset.actor_token).await,
                account(context, fixture_asset.structured_token).await,
            ));
        }
        let assets = asset_range
            .iter()
            .zip(&asset_accounts)
            .map(|(index, (position, mint, actor_shard, structured))| {
                let fixture_asset = fixture.assets.get(*index).expect("asset");
                AssetObservationV2 {
                    outcome: u32::try_from(*index).expect("outcome"),
                    claims_custody_position: observed(fixture_asset.position, position),
                    shard_mint: observed(fixture_asset.mint, mint),
                    actor_shard_account: observed(fixture_asset.actor_token, actor_shard),
                    structured_custody_account: observed(
                        fixture_asset.structured_token,
                        structured,
                    ),
                }
            })
            .collect::<Vec<_>>();
        let observation = RationalObservationV2 {
            caller_role: CallerRoleV2::Trading,
            registry_program: REGISTRY_PROGRAM_ID,
            activation_cache: observed(fixture.activation_cache, &activation),
            descriptor: record(
                REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
                fixture.descriptor_raw,
                &descriptor_raw,
                fixture.descriptor_staging,
                &descriptor_staging,
            ),
            graph: record(
                COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
                fixture.graph_raw,
                &graph_raw,
                fixture.graph_staging,
                &graph_staging,
            ),
            product_evidence: ProductEvidenceObservationV2 {
                linked_basis: record(
                    GRADED_BASIS_RECORD_SCHEMA_ID_V3,
                    fixture.linked_basis_record,
                    &linked_raw,
                    fixture.linked_basis_staging,
                    &linked_staging,
                ),
                product: record(
                    PRODUCT_RECORD_SCHEMA_ID_V2,
                    fixture.product_record,
                    &product_raw,
                    fixture.product_staging,
                    &product_staging,
                ),
                result_domain: record(
                    RESULT_DOMAIN_SCHEMA_ID_V2,
                    fixture.result_domain_record,
                    &domain_raw,
                    fixture.result_domain_staging,
                    &domain_staging,
                ),
                portfolio: record(
                    PORTFOLIO_SCHEMA_ID_V2,
                    fixture.portfolio_record,
                    &portfolio_raw,
                    fixture.portfolio_staging,
                    &portfolio_staging,
                ),
            },
            core_market: observed(fixture.market, &market),
            claims_aggregate: observed(fixture.aggregate, &aggregate),
            replay: ReplayObservationV2 {
                account: observed(fixture.representation_replay, &replay),
            },
            receipt_mint: observed(fixture.receipt_mint, &receipt_mint),
            // THREE SHAPES, NOT TWO. The structured actions carry the actor's
            // receipt; the selected actions carry the actor's Claims Position;
            // a terminal redemption carries NEITHER, and
            // `authenticate_terminal_context` refuses `InvalidAction` if either
            // is present -- it is against the Custody Position and the
            // resolution certificate, not against anything the actor holds in
            // this Market's own books.
            actor_receipt_account: (!selected && !terminal)
                .then_some(observed(fixture.actor_receipt, &actor_receipt)),
            actor_claims_position: (selected && !terminal)
                .then_some(observed(fixture.actor_position, &actor_position)),
            assets: &assets,
            actor: fixture.actor.pubkey(),
            parent_context: [0; 32],
            rent: &Rent::default(),
        };
        match action {
            RepresentationActionV2::RedeemTerminal => {
                let accounts = terminal_fixture.expect("terminal fixture");
                let (realm_raw, realm_staging, certificate, replay, mint, hoard, recipient) =
                    terminal_observed.as_ref().expect("terminal observations");
                HotPlanV3::Terminal(construct_chain_hot_redeem_terminal_v3(
                    observation,
                    TerminalObservationV2 {
                        outcome: WINNER,
                        quantity: 1,
                        realm: record(
                            REALM_SCHEMA_RELEASE_ID_V1,
                            accounts.realm_raw,
                            realm_raw,
                            accounts.realm_staging,
                            realm_staging,
                        ),
                        terminal_certificate: observed(accounts.certificate, certificate),
                        custody_replay: observed(accounts.custody_replay, replay),
                        collateral_mint: observed(accounts.collateral_mint, mint),
                        hoard: observed(accounts.hoard, hoard),
                        collateral_recipient: observed(accounts.recipient, recipient),
                    },
                ))
            }
            RepresentationActionV2::Denominate => HotPlanV3::Selected(
                construct_chain_hot_denominate_v3(
                    observation,
                    SelectedActionInputV2 {
                        outcome: WINNER,
                        quantity: 1,
                    },
                )
                .expect("public Denominate planner"),
            ),
            _ => HotPlanV3::Structured(
                construct_chain_hot_issue_structured_v3(
                    observation,
                    StructuredActionInputV2 { quantity: 1 },
                )
                .expect("public IssueStructured planner"),
            ),
        }
    }

    /// One planned Hot family request, in the three shapes the five actions
    /// compile to.
    pub(super) enum HotPlanV3 {
        Structured(ConstructedHotOpenStructuredV3),
        Selected(ConstructedHotOpenSelectedV3),
        Terminal(Result<ConstructedHotTerminalV3, dclutch_bearer_v2_operator::Error>),
    }

    pub(super) async fn plan_issue(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
    ) -> ConstructedHotOpenStructuredV3 {
        match plan(context, fixture, RepresentationActionV2::IssueStructured).await {
            HotPlanV3::Structured(plan) => plan,
            _ => panic!("Structured plan"),
        }
    }

    pub(super) async fn plan_denominate(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
    ) -> ConstructedHotOpenSelectedV3 {
        match plan(context, fixture, RepresentationActionV2::Denominate).await {
            HotPlanV3::Selected(plan) => plan,
            _ => panic!("selected plan"),
        }
    }

    /// Plan one terminal redemption through the same Hot planner the browser
    /// would use, rather than the direct-caller path every terminal test in
    /// this island has used until now.
    pub(super) async fn plan_redeem(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
    ) -> ConstructedHotTerminalV3 {
        match plan(context, fixture, RepresentationActionV2::RedeemTerminal).await {
            HotPlanV3::Terminal(plan) => plan.expect("public RedeemTerminal planner"),
            _ => panic!("terminal plan"),
        }
    }

    pub(super) async fn token_behavior(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
    ) -> dclutch_rational_representation_v2_contract::AuthenticatedTokenBehaviorV2 {
        let descriptor = account(context, fixture.descriptor_raw).await;
        let admitted =
            dclutch_rational_representation_v2_kernel::RepresentationDescriptorV2::decode(
                &descriptor.data,
                DescriptorAdmissionV2 {
                    selected_descriptor_id: fixture.descriptor_id,
                    finalized_descriptor_id: fixture.descriptor_id,
                    recomputed_descriptor_digest: hash(&descriptor.data).to_bytes(),
                    finalized_descriptor_digest: fixture.descriptor_id,
                    record_authenticated: true,
                    derived_representation_authority: fixture.representation_authority.to_bytes(),
                    authority_derivation_authenticated: true,
                },
            )
            .expect("finalized descriptor admission");
        let release = fixture.hot_release.as_ref().expect("Hot release");
        authenticate_token_behavior_v2(
            admitted,
            fixture.realm_id,
            &release.set.token_behavior_selection,
            TokenBehaviorRecordAdmissionV2 {
                selected_schema_id: dclutch_token_svm::TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                finalized_schema_id: dclutch_token_svm::TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                selected_content_digest: release.set.token_behavior_selection_id,
                finalized_content_digest: release.set.token_behavior_selection_id,
                recomputed_content_digest: hash(&release.set.token_behavior_selection).to_bytes(),
                record_authenticated: true,
                market_realm_authenticated: true,
            },
        )
        .expect("Token behavior admission")
    }

    fn built(key: Pubkey, account: Account) -> BuiltAccountV1 {
        BuiltAccountV1 {
            key,
            account,
            observed: None,
        }
    }

    fn artifact_set<'a>(
        release: &'a HotReleaseFixture,
        action: RepresentationActionV2,
    ) -> ArtifactSetV1<'a> {
        let (descriptor, account_profile, request_profile, transition, effect, lifecycle, strategy) =
            match action {
                RepresentationActionV2::IssueStructured => (
                    release.issue.descriptor.as_slice(),
                    release.issue.account_profile.as_slice(),
                    release.issue.request_profile.as_slice(),
                    release.issue.transition.as_slice(),
                    release.issue.effect.as_slice(),
                    release.issue.lifecycle_policy.as_slice(),
                    release.issue.strategy.as_slice(),
                ),
                RepresentationActionV2::RedeemTerminal => (
                    release.redeem.descriptor.as_slice(),
                    release.redeem.account_profile.as_slice(),
                    release.redeem.request_profile.as_slice(),
                    release.redeem.transition.as_slice(),
                    release.redeem.effect.as_slice(),
                    release.redeem.lifecycle_policy.as_slice(),
                    release.redeem.strategy.as_slice(),
                ),
                RepresentationActionV2::Denominate => (
                    release.denominate.descriptor.as_slice(),
                    release.denominate.account_profile.as_slice(),
                    release.denominate.request_profile.as_slice(),
                    release.denominate.transition.as_slice(),
                    release.denominate.effect.as_slice(),
                    release.denominate.lifecycle_policy.as_slice(),
                    release.denominate.strategy.as_slice(),
                ),
                _ => panic!("physical common-Hot action"),
            };
        ArtifactSetV1 {
            descriptor,
            account_profile,
            request_profile,
            transition,
            effect,
            lifecycle,
            strategy,
            program_set: &release.set.program_set,
            manifest: &release.manifest.bytes,
            config: &release.set.token_behavior_selection,
        }
    }

    pub(super) async fn build_hot(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
        action: RepresentationActionV2,
        family_request: &[u8],
        claims_child: &dclutch_rational_representation_v2_operator::ConstructedInstructionV2,
    ) -> BuiltBundleV1 {
        let release = fixture.hot_release.as_ref().expect("Hot release");
        let set = artifact_set(release, action);
        let tail_count = if action == RepresentationActionV2::IssueStructured {
            OUTCOME_COUNT
        } else {
            0
        };
        let profile =
            dclutch_account_profile_contract::v2::AccountProfileV2::decode(set.account_profile)
                .expect("AccountProfile");
        let staging_cursors = [
            fixture.descriptor_staging,
            fixture.graph_staging,
            fixture.linked_basis_staging,
            fixture.product_staging,
            fixture.result_domain_staging,
            fixture.portfolio_staging,
        ];
        let mut bindings = Vec::new();
        for (child_index, meta) in claims_child.instruction.accounts.iter().enumerate() {
            let logical = 5_usize
                .checked_add(child_index)
                .expect("logical coordinate");
            let representative = profile
                .representative_with_dynamic_spans(tail_count, &[], logical)
                .expect("profile representative");
            if representative != logical || child_index == 0 {
                continue;
            }
            // A staging cursor is the one child coordinate allowed to hold
            // nothing, because a finalized record's cursor is absent by
            // definition (`add_finalized_record`). Every other coordinate must
            // exist, and `account` still refuses by name when it does not.
            //
            // NOT "bind whatever is there": that was tried on 2026-09-02 and it
            // silently changed the ISSUE frame. Several coordinates the fixture
            // supplies are absent at bind time and the builder derives its own
            // address for an unbound one, so skipping them substituted six
            // accounts in a frame that had been committing. The vacancy is
            // granted to the addresses that earned it, never to the loop.
            // THREE TERMINAL COORDINATES HOLD NOTHING BY CONSTRUCTION, and they
            // are granted the vacancy BY ROLE rather than by address. The two
            // Custody PDAs -- the caller authority the child invokes through
            // and the authority that owns the Hoard -- are derived addresses
            // that only ever sign; nothing creates them, and the child frame's
            // own coordinate 0 is the same shape, which is why the loop already
            // skips it. The Realm's staging cursor is a finalized record's
            // cursor, which `authenticate_terminal_privileges` requires to be
            // System-owned and empty -- so an account there would be the defect.
            let vacant_by_role = matches!(
                frame_role(action, child_index),
                Some(
                    RepresentationCoordinateV2::TerminalCallerAuthority
                        | RepresentationCoordinateV2::TerminalCustodyAuthority
                        | RepresentationCoordinateV2::TerminalRealmStaging
                )
            );
            let account = if vacant_by_role || staging_cursors.contains(&meta.pubkey) {
                vacant_or_account(context, meta.pubkey).await
            } else {
                named_account(context, meta.pubkey, action, child_index).await
            };
            bindings.push((logical, built(meta.pubkey, account)));
        }

        let market_account = account(context, fixture.market).await;

        let root_key = fixture.capability_root.expect("Structured capability root");
        let root_account = account(context, root_key).await;
        let product = account(context, fixture.product_record).await;
        let domain = account(context, fixture.result_domain_record).await;
        let portfolio = account(context, fixture.portfolio_record).await;
        let basis = account(context, fixture.linked_basis_record).await;
        let product_record = derive_record(
            REGISTRY_PROGRAM_ID,
            PRODUCT_RECORD_SCHEMA_ID_V2,
            &product.data,
        );
        let domain_record = derive_record(
            REGISTRY_PROGRAM_ID,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            &domain.data,
        );
        let portfolio_record =
            derive_record(REGISTRY_PROGRAM_ID, PORTFOLIO_SCHEMA_ID_V2, &portfolio.data);
        let basis_record = derive_record(
            REGISTRY_PROGRAM_ID,
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            &basis.data,
        );
        assert_eq!(product_record.raw, fixture.product_record);
        assert_eq!(domain_record.raw, fixture.result_domain_record);
        assert_eq!(portfolio_record.raw, fixture.portfolio_record);
        assert_eq!(basis_record.raw, fixture.linked_basis_record);

        let trading = trading_artifact();
        let trading_release = super::release(TRADING_PROGRAM_ID, 0x44, &trading);
        let waist = WaistFactsV1 {
            registry_program: REGISTRY_PROGRAM_ID,
            trading_program: TRADING_PROGRAM_ID,
            core_program: CORE_PROGRAM_ID,
            claims_program: CLAIMS_PROGRAM_ID,
            custody_program: CUSTODY_PROGRAM_ID,
            release_set: fixture.release_set,
            activation_cache: fixture.activation_cache,
            trading_semantic_release: trading_release.semantic_release_id().to_bytes(),
        };
        let extras = [TOKEN_PROGRAM_ID];
        let rent = Rent::default();
        let input = BundleInputV1 {
            set,
            waist,
            scenario: ScenarioV1 {
                family_request,
                tail_count,
                clock_slot: 1,
                generation: GENERATION,
                ed25519_evidence: None,
                native_message_instruction_index: 0,
                externally_installed_extra: &extras,
                payer: context.payer.pubkey(),
            },
            fixed: FixedCorpusV1 {
                market: built(fixture.market, market_account),
                root: built(root_key, root_account),
                product: product_record,
                result_domain: domain_record,
                portfolio: portfolio_record,
                linked_basis: basis_record,
                core_programdata: fixture.core_programdata,
                trading_programdata: fixture.caller_programdata,
            },
            bindings: &bindings,
            rent: &rent,
        };
        let built = build_bundle(&input).expect("shared common-Hot bundle builder");
        let authority = built.authorities.first().expect("Claims caller authority");
        assert_eq!(
            authority.authority,
            claims_child
                .instruction
                .accounts
                .first()
                .expect("child caller")
                .pubkey
        );
        assert_eq!(authority.request_digest, claims_child.request_digest);
        built
    }

    pub(super) fn install(context: &mut ProgramTestContext, bundle: &BuiltBundleV1) {
        for install in &bundle.accounts {
            if !bundle.externally_installed_keys.contains(&install.key) {
                context.set_account(
                    &install.key,
                    &AccountSharedData::from(install.account.clone()),
                );
            }
        }
    }

    fn hot_state<'a>(
        fixture: &Fixture,
        root_data: &'a [u8],
        fixed_accounts: &'a [AccountMeta],
    ) -> dclutch_bearer_v2_operator::RationalTerminalHotStateV3<'a> {
        let trading = trading_artifact();
        let release = super::release(TRADING_PROGRAM_ID, 0x44, &trading);
        dclutch_bearer_v2_operator::RationalTerminalHotStateV3 {
            fixed_accounts,
            strategy_accounts: &[],
            root_data,
            release_set: fixture.release_set,
            market: fixture.market,
            generation: GENERATION,
            finalized_slot: 1,
            hot_outer: Some(CheckedRationalHotOuterReleaseV3 {
                trading_program: TRADING_PROGRAM_ID,
                artifact_release: super::artifact_id(release).to_bytes(),
                checked_manifest_digest: hash(
                    &fixture
                        .hot_release
                        .as_ref()
                        .expect("Hot release")
                        .manifest
                        .bytes,
                )
                .to_bytes(),
            }),
        }
    }

    pub(super) async fn assert_public_issue_outer(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
        planned: &ConstructedHotOpenStructuredV3,
        built: &BuiltBundleV1,
    ) {
        let root = account(context, fixture.capability_root.expect("root")).await;
        let fixed = built
            .hot_instruction
            .accounts
            .get(..dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3)
            .expect("Hot fixed prefix");
        let state: RationalOpenStructuredHotStateV3<'_> = hot_state(fixture, &root.data, fixed);
        let behavior = token_behavior(context, fixture).await;
        let release = fixture.hot_release.as_ref().expect("Hot release");
        let public = build_rational_open_structured_hot_instruction_v3(
            &state,
            planned,
            &release.issue,
            &release.set,
            behavior,
        )
        .expect("public complete IssueStructured outer");
        assert_eq!(public.instruction, built.hot_instruction);
        assert_eq!(public.required_wallet_signers, vec![fixture.actor.pubkey()]);
    }

    pub(super) async fn assert_public_redeem_outer(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
        planned: &ConstructedHotTerminalV3,
        built: &BuiltBundleV1,
    ) {
        let root = account(context, fixture.capability_root.expect("root")).await;
        let fixed = built
            .hot_instruction
            .accounts
            .get(..dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3)
            .expect("Hot fixed prefix");
        let state = hot_state(fixture, &root.data, fixed);
        let behavior = token_behavior(context, fixture).await;
        let release = fixture.hot_release.as_ref().expect("Hot release");
        let public = build_rational_terminal_hot_instruction_v3(
            &state,
            planned,
            &release.redeem,
            &release.set,
            behavior,
        )
        .expect("public complete RedeemTerminal outer");
        assert_eq!(public.instruction, built.hot_instruction);
        assert_eq!(public.required_wallet_signers, vec![fixture.actor.pubkey()]);
    }

    pub(super) async fn assert_public_denominate_outer(
        context: &mut ProgramTestContext,
        fixture: &Fixture,
        planned: &ConstructedHotOpenSelectedV3,
        built: &BuiltBundleV1,
    ) {
        let root = account(context, fixture.capability_root.expect("root")).await;
        let fixed = built
            .hot_instruction
            .accounts
            .get(..dclutch_capability_program_contract::hot_v3::HOT_FIXED_ACCOUNT_COUNT_V3)
            .expect("Hot fixed prefix");
        let state: RationalOpenSelectedHotStateV3<'_> = hot_state(fixture, &root.data, fixed);
        let behavior = token_behavior(context, fixture).await;
        let release = fixture.hot_release.as_ref().expect("Hot release");
        let public = build_rational_open_selected_hot_instruction_v3(
            &state,
            planned,
            &release.denominate,
            &release.set,
            behavior,
        )
        .expect("public complete Denominate outer");
        assert_eq!(public.instruction, built.hot_instruction);
        assert_eq!(public.required_wallet_signers, vec![fixture.actor.pubkey()]);
    }
}

fn token_2022_provenance_value(key: &str) -> &'static str {
    let prefix = format!("{key}=");
    let mut values = TOKEN_2022_V11_PROVENANCE
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix));
    let value = values.next().expect("Token-2022 provenance key");
    assert!(
        values.next().is_none(),
        "duplicate Token-2022 provenance key: {key}"
    );
    value
}

fn decode_sha256(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64, "SHA-256 must contain 64 hex digits");
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let digits = str::from_utf8(pair).expect("ASCII SHA-256 digits");
        *output.get_mut(index).expect("SHA-256 byte") =
            u8::from_str_radix(digits, 16).expect("hex SHA-256 digit");
    }
    output
}

fn expected_token_2022_v11_elf_digest() -> [u8; 32] {
    decode_sha256(token_2022_provenance_value("canonical_elf_sha256"))
}

fn token_2022_v11_fixture_authenticates(bytes: &[u8]) -> bool {
    hash(bytes).to_bytes() == expected_token_2022_v11_elf_digest()
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |name: &str| {
        let path = directory.join(name);
        assert!(path.is_file(), "missing real ELF: {}", path.display());
        fs::read(path).expect("read real ELF")
    };
    let token_2022 = read("spl_token_2022.so");
    assert!(
        token_2022_v11_fixture_authenticates(&token_2022),
        "the matching real Token-2022 v11 runtime is required"
    );
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        custody: read("dclutch_custody_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        resolution: read("dclutch_resolution_proof_sbf.so"),
        token_2022,
        caller: read("dclutch_rational_v2_test_caller_sbf.so"),
    }
}

fn trading_artifact() -> Vec<u8> {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let path = directory.join("dclutch_trading_sbf.so");
    assert!(path.is_file(), "missing real ELF: {}", path.display());
    fs::read(path).expect("read real Trading ELF")
}

#[test]
fn token_2022_v11_fixture_refuses_every_sampled_byte_substitution() {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let mut bytes = fs::read(directory.join("spl_token_2022.so")).expect("read Token-2022 ELF");
    assert!(token_2022_v11_fixture_authenticates(&bytes));
    assert_ne!(
        expected_token_2022_v11_elf_digest(),
        decode_sha256(token_2022_provenance_value("macos_arm64_audit_elf_sha256")),
        "the cross-host audit artifact must not become an accepted substitute"
    );

    let last = bytes.len().checked_sub(1).expect("nonempty Token-2022 ELF");
    for offset in [0, bytes.len() / 2, last] {
        let byte = bytes.get_mut(offset).expect("sampled ELF byte");
        *byte ^= 1;
        assert!(!token_2022_v11_fixture_authenticates(&bytes));
        let byte = bytes.get_mut(offset).expect("sampled ELF byte");
        *byte ^= 1;
    }
    assert!(token_2022_v11_fixture_authenticates(&bytes));
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    let end = offset.checked_add(input.len()).expect("fixture offset");
    output
        .get_mut(offset..end)
        .expect("fixture field")
        .copy_from_slice(input);
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

fn identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("nonzero program identity")
}

fn semantic_identity(bytes: [u8; 32]) -> Identity {
    Identity::new(bytes).expect("nonzero semantic identity")
}

fn market_rent_credit() -> Pubkey {
    Pubkey::new_from_array([0x65; 32])
}

fn programdata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 45 + elf.len()];
    put(&mut bytes, 0, &3_u32.to_le_bytes());
    put(&mut bytes, 4, &0_u64.to_le_bytes());
    *bytes.get_mut(12).expect("ProgramData authority option") = 0;
    put(&mut bytes, 45, elf);
    bytes
}

fn add_account(test: &mut ProgramTest, key: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_funded_empty(test: &mut ProgramTest, key: Pubkey, required_bytes: usize) {
    test.add_account(
        key,
        Account {
            lamports: Rent::default().minimum_balance(required_bytes).max(1),
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn add_upgradeable_program(
    test: &mut ProgramTest,
    name: &'static str,
    program: Pubkey,
    elf: &[u8],
) {
    test.add_upgradeable_program_to_genesis(name, &program);
    add_account(
        test,
        programdata_address(program),
        bpf_loader_upgradeable::ID,
        immutable_programdata(elf),
    );
}

fn release(program: Pubkey, semantic_seed: u8, elf: &[u8]) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        identity(program),
        identity(bpf_loader_upgradeable::ID),
        programdata_address(program).to_bytes(),
        ContentId::new([semantic_seed; 32]).expect("semantic release"),
        hash(elf).to_bytes(),
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release")
}

fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact ID")
}

fn binding(release: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
}

fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    let observation = DeploymentObservationV1::new(
        release.program().to_bytes(),
        bpf_loader_upgradeable::ID.to_bytes(),
        true,
        release.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        false,
        release.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        release.deployment_slot(),
        release.elf_digest(),
        release.upgrade_authority(),
    )
    .expect("deployment observation");
    ArtifactActivationInputV1::new(artifact_id(release), release, observation)
}

fn activation_cache(artifacts: &Artifacts) -> ([u8; 32], Vec<u8>) {
    activation_cache_for_upstream(artifacts, TEST_CALLER_PROGRAM_ID, &artifacts.caller)
}

fn activation_cache_for_upstream(
    artifacts: &Artifacts,
    upstream_program: Pubkey,
    upstream_elf: &[u8],
) -> ([u8; 32], Vec<u8>) {
    let core = release(CORE_PROGRAM_ID, 0x41, &artifacts.core);
    let claims = release(CLAIMS_PROGRAM_ID, 0x42, &artifacts.claims);
    let custody = release(CUSTODY_PROGRAM_ID, 0x43, &artifacts.custody);
    let trading = release(upstream_program, 0x44, upstream_elf);
    let resolution = release(RESOLUTION_PROGRAM_ID, 0x45, &artifacts.resolution);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core),
        binding(claims),
        binding(trading),
        binding(resolution),
        binding(custody),
    )
    .expect("release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release-set ID");
    let mut bytes = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut bytes, content).expect("initialize cache");
    for (role, artifact) in [
        (ExecutionRoleV1::Core, core),
        (ExecutionRoleV1::Claims, claims),
        (ExecutionRoleV1::Trading, trading),
        (ExecutionRoleV1::Resolution, resolution),
        (ExecutionRoleV1::Custody, custody),
    ] {
        activate_execution_role_into_v1(
            &mut bytes,
            content,
            &release_set,
            role,
            &activation_input(artifact),
        )
        .expect("activate role");
    }
    ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete cache");
    (release_set_id, bytes)
}

fn finalized_record_keys(schema: [u8; 32], digest: [u8; 32]) -> (Pubkey, Pubkey) {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).expect("schema release id"),
        ContentDigest::new(digest).expect("content digest"),
    );
    let address = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &REGISTRY_PROGRAM_ID,
        )
        .0
    };
    (
        address(key.raw_record_pda_seeds()),
        address(key.staging_cursor_pda_seeds()),
    )
}

fn add_finalized_record(
    test: &mut ProgramTest,
    schema: [u8; 32],
    bytes: &[u8],
) -> (Pubkey, Pubkey, [u8; 32]) {
    let digest = hash(bytes).to_bytes();
    let (raw, staging) = finalized_record_keys(schema, digest);
    add_account(test, raw, REGISTRY_PROGRAM_ID, bytes.to_vec());
    // THE STAGING CURSOR IS NOT INSTALLED, because a finalized record does not
    // have one. This function used to fund it with `Rent::minimum_balance(0)`,
    // which is the PREFUNDED, NOT-YET-BEGUN state -- the one state a record
    // called `finalized` cannot be in -- and its own name said so.
    //
    // The chain closes the cursor at finalization and says what that means in
    // three places. `process_finalize` drains it to the refund wallet and then
    // re-reads it, refusing unless `is_vacant`: System-owned, empty data, and
    // ZERO LAMPORTS (`registry-sbf/src/record_v1.rs:493-509,986-994`). The
    // record contract makes absence the DEFINITION rather than a consequence --
    // `authenticate_finalized_raw_record_v1` is documented as "the adapter must
    // prove the canonical staging PDA is absent ... thus the raw account's PDA
    // and apparent payload alone never assert finality" (`lib.rs:1566-1571`).
    // And Trading says the same from the consumer side: the artifact seal "is
    // the durable proof that the real staging cursor was vacant when this exact
    // raw body was admitted" (`hot_v3.rs:3290-3293`).
    //
    // Two authors stated this account's contents and only one of them could be
    // right. The shared bundle builder pairs `finalized_raw(rent, record)` with
    // `vacant(record.staging)` (`bundle-builder/src/bundle.rs:1057-1059`), which
    // is not a restatement of a chain computation but the definition of the
    // pair; `common_hot_open::install` wrote that model into the bank, the
    // fabricated 890,880 lamports went to zero, and the account was purged
    // before any transaction ran. Measured on 2026-09-01, both sides of the
    // step. This is the fixture withdrawing its claim, not the model changing.
    (raw, staging, digest)
}

/// The Structured basis this campaign lowers onto the Rational wire.
///
/// Every identity here is a distinct record. Two of them used to be one value,
/// and separating them is the whole of decision 0011 §3d's first correction:
/// [`EXPOSURE_ID`] is the finalized exposure bundle the descriptor names, and
/// [`SOURCE_GRAPH_ID`] is the composition DAG that bundle was projected from.
fn campaign_basis(
    market: Pubkey,
    release_set: [u8; 32],
    receipt_mint: Pubkey,
    product: &ProductClaimsFixture,
) -> StructuredBasis {
    StructuredBasis {
        market: market.to_bytes(),
        product_record: product.product_digest,
        result_domain: product.result_domain_id,
        release_set,
        product_basis: product.linked_basis_digest,
        representation_basis: product.basis_id,
        exposure_id: EXPOSURE_ID,
        source_graph_id: SOURCE_GRAPH_ID,
        root_id: COMPOSITION_ROOT_ID,
        translation_id: CANONICAL_TRANSLATION_ID,
        receipt_mint: receipt_mint.to_bytes(),
        token_program: TOKEN_2022_PROGRAM_ID,
        shard_token_behavior: SHARD_TOKEN_BEHAVIOR_ID,
        receipt_token_behavior: RECEIPT_TOKEN_BEHAVIOR_ID,
        shard_mints: (0..COEFFICIENTS.len())
            .map(|index| {
                let mut value = [0_u8; 32];
                value[0] = 0xd0;
                value[1] = u8::try_from(index).expect("shard Mint index");
                value[31] = 0xa5;
                value
            })
            .collect(),
        coefficients: COEFFICIENTS.to_vec(),
        denominator: DENOMINATOR,
        product_width: OUTCOME_COUNT,
    }
}

/// A descriptor the derivation would never mint: the same composition, a
/// different recipe.
///
/// This is the campaign's same-width descriptor hostile, and it is built by
/// MUTATING the derived preimage rather than by hand-filling a seventh one.
/// The mutation is exactly the field `require_coefficients_are_the_composition_root`
/// exists to bind, so the hostile and the join name the same fact.
fn descriptor_with_substituted_coefficients(preimage: &[u8], coefficients: &[u64]) -> Vec<u8> {
    let mut bytes = preimage.to_vec();
    for (index, coefficient) in coefficients.iter().enumerate() {
        put_u64(
            &mut bytes,
            DESCRIPTOR_HEADER_BYTES + index * DESCRIPTOR_COEFFICIENT_BYTES,
            *coefficient,
        );
    }
    assert_ne!(bytes, preimage, "the near-miss must differ from the record");
    bytes
}

fn core_market(
    release_set: [u8; 32],
    realm_id: [u8; 32],
    product_record: [u8; 32],
    product_id: [u8; 32],
    terminal_receipt: Option<[u8; 32]>,
    terminal_winner: u32,
    capability_manifest: [u8; 32],
) -> (Pubkey, Vec<u8>) {
    let mut identity = MarketIdentity {
        market_id: semantic_identity([1; 32]),
        realm_id: semantic_identity(realm_id),
        product_record: semantic_identity(product_record),
        product_id: semantic_identity(product_id),
        resolution_policy: semantic_identity([0x63; 32]),
        capability_manifest: semantic_identity(capability_manifest),
        selected_release_set: semantic_identity(release_set),
        registry_program: semantic_identity(REGISTRY_PROGRAM_ID.to_bytes()),
        generation: GENERATION,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .0;
    identity.market_id = semantic_identity(market.to_bytes());
    let state = CoreState {
        phase: if terminal_receipt.is_some() {
            CorePhase::Terminal
        } else {
            CorePhase::Open
        },
        readiness: Readiness::Consumed,
        terminal_winner: if terminal_receipt.is_some() {
            terminal_winner
        } else {
            0
        },
        identity,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: semantic_identity(market_rent_credit().to_bytes()),
        terminal_receipt: terminal_receipt.map(semantic_identity),
        bumps: StateBumpsV1::UNRECORDED,
    };
    (market, state.encode().expect("Core state").to_vec())
}

struct ProductClaimsFixture {
    product_id: [u8; 32],
    product_digest: [u8; 32],
    result_domain_id: [u8; 32],
    basis_id: [u8; 32],
    linked_basis_bytes: Vec<u8>,
    linked_basis_digest: [u8; 32],
    linked_basis_record: Pubkey,
    linked_basis_staging: Pubkey,
    product_record: Pubkey,
    product_staging: Pubkey,
    result_domain_record: Pubkey,
    result_domain_staging: Pubkey,
    portfolio_record: Pubkey,
    portfolio_staging: Pubkey,
    payout_scale: u64,
}

fn runtime_id(value: [u8; 32]) -> RuntimeContentId {
    RuntimeContentId::new(value).expect("runtime identity")
}

fn add_product_claims(
    test: &mut ProgramTest,
    profile: ProductBasisProfileV1,
) -> ProductClaimsFixture {
    let stable_product = [0x61; 32];
    let product_id = runtime_id(stable_product);
    let coordinate_domain_id = runtime_id([0x62; 32]);
    let result_unit_id = runtime_id([0x63; 32]);
    let kind = profile.kind();
    let knots = profile.knots();
    let failure_payouts = profile.failure_payouts();
    let record_bytes = basis_record_bytes_v3(kind, OUTCOME_COUNT as usize, knots.len(), 0)
        .expect("ProductBasisV3 width");

    // A spline basis must name a DCLTPGT1 certificate digest before it can be
    // compiled. Build the certificate from a probe whose only provisional fact
    // is that digest: the evaluator does not read the digest, and the final pass
    // below binds the hash of the exact verified certificate bytes.
    let price_gate_certificate_digest = match profile {
        ProductBasisProfileV1::Categorical => [0_u8; 32],
        ProductBasisProfileV1::CurvedDegreeTwo => {
            let degree = profile.spline_degree().expect("curved degree");
            let mut probe = vec![0_u8; record_bytes];
            compile_basis_v3(
                BasisInputV3 {
                    kind,
                    product_id: product_id.to_bytes(),
                    result_domain_id: [0x67; 32],
                    coordinate_domain_id: coordinate_domain_id.to_bytes(),
                    result_unit_id: result_unit_id.to_bytes(),
                    evaluator_release_id: [0x68; 32],
                    basis_width: OUTCOME_COUNT,
                    payout_scale: profile.payout_scale(),
                    knot_denominator: profile.knot_denominator(),
                    knots: &knots,
                    terms: &[],
                    failure_payouts: &failure_payouts,
                    price_gate_certificate_digest: [1_u8; 32],
                },
                &mut probe,
            )
            .expect("curved probe basis");
            let basis = ProductBasisV3::decode(&probe).expect("curved probe decodes");
            let mut payouts = [0_u64; K];
            basis
                .evaluate_rational(
                    CURVED_RESULT_NUMERATOR,
                    CURVED_RESULT_DENOMINATOR,
                    &mut payouts,
                )
                .expect("curved terminal coordinate evaluates");
            assert_eq!(
                payouts,
                profile.expected_curve_payouts(),
                "cumulative-floor is observable"
            );

            let mut certificate = [0_u8; PRICE_GATE_REQUEST_BYTES_V1];
            certificate[PRICE_GATE_MAGIC_OFFSET_V1..PRICE_GATE_MAGIC_OFFSET_V1 + 8]
                .copy_from_slice(&PRICE_GATE_MAGIC_V1);
            certificate[PRICE_GATE_VERSION_OFFSET_V1..PRICE_GATE_VERSION_OFFSET_V1 + 2]
                .copy_from_slice(&PRICE_GATE_SCHEMA_VERSION_V1.to_le_bytes());
            certificate[PRICE_GATE_PROFILE_OFFSET_V1..PRICE_GATE_PROFILE_OFFSET_V1 + 2]
                .copy_from_slice(&PRICE_GATE_PROFILE_V1.to_le_bytes());
            certificate[PRICE_GATE_SCALE_OFFSET_V1..PRICE_GATE_SCALE_OFFSET_V1 + 4]
                .copy_from_slice(
                    &u32::try_from(profile.payout_scale())
                        .expect("price-gate scale")
                        .to_le_bytes(),
                );
            certificate[PRICE_GATE_MASS_OFFSET_V1..PRICE_GATE_MASS_OFFSET_V1 + 8]
                .copy_from_slice(&1_u64.to_le_bytes());
            certificate[PRICE_GATE_DEGREE_OFFSET_V1] = degree;
            certificate[PRICE_GATE_WIDTH_OFFSET_V1] =
                u8::try_from(OUTCOME_COUNT).expect("price-gate width");
            certificate[PRICE_GATE_ATOM_COUNT_OFFSET_V1] = 1;
            for (claim, payout) in payouts.iter().enumerate() {
                let offset = PRICE_GATE_PRICES_OFFSET_V1 + claim * 8;
                certificate[offset..offset + 8].copy_from_slice(&payout.to_le_bytes());
            }
            certificate[PRICE_GATE_WEIGHTS_OFFSET_V1..PRICE_GATE_WEIGHTS_OFFSET_V1 + 8]
                .copy_from_slice(&1_u64.to_le_bytes());
            certificate[PRICE_GATE_NUMERATORS_OFFSET_V1..PRICE_GATE_NUMERATORS_OFFSET_V1 + 8]
                .copy_from_slice(
                    &i64::try_from(CURVED_RESULT_NUMERATOR)
                        .expect("price-gate atom numerator")
                        .to_le_bytes(),
                );
            certificate[PRICE_GATE_DENOMINATORS_OFFSET_V1..PRICE_GATE_DENOMINATORS_OFFSET_V1 + 4]
                .copy_from_slice(
                    &u32::try_from(CURVED_RESULT_DENOMINATOR)
                        .expect("price-gate atom denominator")
                        .to_le_bytes(),
                );
            let verified = verify_price_gate_v1(
                &basis,
                basis.knot_denominator(),
                basis.payout_scale(),
                degree,
                basis.basis_width(),
                &certificate,
            )
            .expect("one-atom price-gate certificate verifies");
            assert_eq!(verified.active_prices(), payouts.as_slice());
            hash(&certificate).to_bytes()
        }
    };
    let provisional_input = BasisInputV3 {
        kind,
        product_id: product_id.to_bytes(),
        result_domain_id: [0x67; 32],
        coordinate_domain_id: coordinate_domain_id.to_bytes(),
        result_unit_id: result_unit_id.to_bytes(),
        evaluator_release_id: [0x68; 32],
        basis_width: OUTCOME_COUNT,
        payout_scale: profile.payout_scale(),
        knot_denominator: profile.knot_denominator(),
        knots: &knots,
        terms: &[],
        failure_payouts: &failure_payouts,
        price_gate_certificate_digest,
    };
    let mut provisional = vec![0_u8; record_bytes];
    compile_basis_v3(provisional_input, &mut provisional).expect("provisional ProductBasisV3");
    let semantic =
        semantic_basis_preimage_v3(&provisional).expect("ProductBasisV3 semantic preimage");
    let basis_id = hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes();

    // `ResultDomainV2::outcome_count()` is `cuts + 2`, and `join_product_v2`
    // requires it to equal the portfolio's coefficient count. So the domain's
    // cut count is `K - 2`, not zero: at the K = 2 basis this campaign used to
    // run, an EMPTY cut list happened to be right, and the identity was
    // invisible until the width moved.
    let cuts: Vec<i128> = (0..K.checked_sub(2).expect("K >= 2"))
        .map(|index| i128::try_from(index).expect("cut numerator"))
        .collect();
    let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain width")];
    compile_result_domain_v2(
        ResultDomainInputV2 {
            product_id,
            coordinate_domain_id,
            result_unit_id,
            liability_basis_id: runtime_id(basis_id),
            representation_release_id: runtime_id([0x64; 32]),
            mapping_release_id: runtime_id([0x65; 32]),
            cut_denominator: 1,
            cuts: &cuts,
        },
        &mut domain,
    )
    .expect("domain");
    let (result_domain_record, result_domain_staging, domain_digest) =
        add_finalized_record(test, RESULT_DOMAIN_SCHEMA_ID_V2, &domain);
    let mut portfolio = vec![0_u8; portfolio_record_bytes(K).expect("portfolio width")];
    compile_portfolio_v2(
        PortfolioInputV2 {
            product_id,
            result_domain_id: runtime_id(domain_digest),
            claim_basis_id: runtime_id([0x66; 32]),
            liability_basis_id: runtime_id(basis_id),
            representation_release_id: runtime_id([0x64; 32]),
            denominator: DENOMINATOR,
            coefficients: &COEFFICIENTS,
        },
        &mut portfolio,
    )
    .expect("portfolio");
    let (portfolio_record, portfolio_staging, portfolio_digest) =
        add_finalized_record(test, PORTFOLIO_SCHEMA_ID_V2, &portfolio);
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    ProductRecordV2::new(
        runtime_id(stable_product),
        runtime_id(domain_digest),
        runtime_id(portfolio_digest),
    )
    .encode_into(&mut product)
    .expect("Product root");
    let (product_record, product_staging, product_digest) =
        add_finalized_record(test, PRODUCT_RECORD_SCHEMA_ID_V2, &product);
    let mut linked = vec![0_u8; record_bytes];
    compile_basis_v3(
        BasisInputV3 {
            result_domain_id: domain_digest,
            ..provisional_input
        },
        &mut linked,
    )
    .expect("ProductBasisV3");
    let (linked_basis_record, linked_basis_staging, linked_basis_digest) =
        add_finalized_record(test, GRADED_BASIS_RECORD_SCHEMA_ID_V3, &linked);
    ProductClaimsFixture {
        product_id: stable_product,
        product_digest,
        result_domain_id: domain_digest,
        basis_id,
        linked_basis_bytes: linked,
        linked_basis_digest,
        linked_basis_record,
        linked_basis_staging,
        product_record,
        product_staging,
        result_domain_record,
        result_domain_staging,
        portfolio_record,
        portfolio_staging,
        payout_scale: profile.payout_scale(),
    }
}

fn mint_data(authority: COption<Pubkey>, supply: u64, decimals: u8) -> Vec<u8> {
    let mut bytes = vec![0; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: authority,
            supply,
            decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack Mint");
    bytes
}

/// Which Token-2022 roles one founding configured on the receipt Mint.
///
/// Decision 0011 §3b, third cost: the representation authority is adopted in
/// TWO Token-2022 roles, not one. It is the Mint authority for `MintReceipt`
/// (`mint_to_checked`) AND the permissioned-burn authority for `BurnReceipt`
/// (`permissioned_burn_instruction::burn_checked`, where the PDA is the burn
/// authority and the ACTOR is the token owner). "Founding must configure both
/// roles on the receipt Mint, or `BurnReceipt` fails at the Token program with
/// the descriptor already committed." That sentence is a claim until a founding
/// configures one role and a real Token-2022 refuses the other.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReceiptMintRoles {
    /// Mint authority and permissioned-burn authority: a correct founding.
    Both,
    /// Only the Mint authority: §3b's under-configured founding.
    MintAuthorityOnly,
}

fn claim_mint_data(authority: Pubkey, supply: u64, decimals: u8) -> Vec<u8> {
    mint_with_roles(authority, supply, decimals, ReceiptMintRoles::Both)
}

fn mint_with_roles(
    authority: Pubkey,
    supply: u64,
    decimals: u8,
    roles: ReceiptMintRoles,
) -> Vec<u8> {
    const BASE_ACCOUNT_BYTES: usize = 165;
    const ACCOUNT_TYPE_OFFSET: usize = BASE_ACCOUNT_BYTES;
    const TLV_START_OFFSET: usize = 166;
    const MINT_CLOSE_AUTHORITY_EXTENSION: u16 = 3;
    const PERMISSIONED_BURN_EXTENSION: u16 = 28;

    let mut bytes = vec![0; TLV_START_OFFSET];
    SplMint::pack(
        SplMint {
            mint_authority: COption::Some(authority),
            supply,
            decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        bytes.get_mut(..SplMint::LEN).expect("base Mint width"),
    )
    .expect("pack claim Mint");
    put(
        &mut bytes,
        ACCOUNT_TYPE_OFFSET,
        &[spl_token_2022_interface::extension::AccountType::Mint as u8],
    );
    append_mint_authority_extension(&mut bytes, MINT_CLOSE_AUTHORITY_EXTENSION, authority);
    if roles == ReceiptMintRoles::Both {
        append_mint_authority_extension(&mut bytes, PERMISSIONED_BURN_EXTENSION, authority);
    }
    bytes
}

fn append_mint_authority_extension(output: &mut Vec<u8>, extension: u16, authority: Pubkey) {
    output.extend_from_slice(&extension.to_le_bytes());
    output.extend_from_slice(&32_u16.to_le_bytes());
    output.extend_from_slice(authority.as_ref());
}

fn token_account_data(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let mut bytes = vec![0; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint,
            owner,
            amount,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut bytes,
    )
    .expect("pack Token Account");
    bytes
}

#[allow(clippy::too_many_arguments)]
fn request_bytes_from(
    action: RepresentationActionV2,
    release_set: [u8; 32],
    market: Pubkey,
    graph_id: [u8; 32],
    descriptor_id: [u8; 32],
    parent_context: [u8; 32],
    actor: Pubkey,
    receipt_mint: Pubkey,
    actor_receipt: Pubkey,
    representation_authority: Pubkey,
    realm_id: [u8; 32],
    recipient: Option<Pubkey>,
    representation_revision: u64,
    receipt_supply: u64,
    actor_balances: [u64; K],
    structured_balances: [u64; K],
    assets: [AssetFixture; K],
    selected_outcome: u32,
) -> Vec<u8> {
    let structured = matches!(
        action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    let terminal = action == RepresentationActionV2::RedeemTerminal;
    let selected_action = action.selected_outcome();
    let selected = if selected_action {
        selected_outcome
    } else {
        u32::MAX
    };
    let asset_count = if selected_action { 1 } else { OUTCOME_COUNT };
    let mut rows = vec![0; usize::try_from(asset_count).expect("asset width") * ASSET_BYTES_V3];
    let requested = if selected_action {
        vec![(
            selected_outcome,
            *assets
                .get(usize::try_from(selected_outcome).expect("selected outcome index"))
                .expect("selected outcome asset"),
        )]
    } else {
        assets
            .iter()
            .enumerate()
            .map(|(index, asset)| (u32::try_from(index).expect("outcome"), *asset))
            .collect()
    };
    for (row, (outcome, asset)) in requested.into_iter().enumerate() {
        let index = usize::try_from(outcome).expect("outcome index");
        AssetV2 {
            shard_mint: asset.mint.to_bytes(),
            actor_shard_account: asset.actor_token.to_bytes(),
            structured_custody_account: asset.structured_token.to_bytes(),
            claims_custody_owner: asset.custody_owner.to_bytes(),
            coefficient: *COEFFICIENTS.get(index).expect("coefficient"),
            expected_shard_supply: shard_supply(index)
                .checked_add(
                    if action == RepresentationActionV2::Reconstitute && outcome == WINNER {
                        DENOMINATOR
                    } else {
                        0
                    },
                )
                .expect("fixture supply"),
            expected_actor_shards: *actor_balances.get(index).expect("actor balance"),
            expected_structured_shards: *structured_balances
                .get(index)
                .expect("structured balance"),
        }
        .encode_into(
            rows.get_mut(row * ASSET_BYTES_V3..(row + 1) * ASSET_BYTES_V3)
                .expect("asset row"),
        )
        .expect("encode asset");
    }
    let request = RepresentationRequestV2::new(
        RepresentationRequestHeaderV2 {
            action,
            caller_role: CallerRoleV2::Trading,
            release_set,
            market: market.to_bytes(),
            graph_id,
            descriptor_id,
            parent_context,
            actor: actor.to_bytes(),
            receipt_mint: receipt_mint.to_bytes(),
            receipt_account: if structured {
                actor_receipt.to_bytes()
            } else {
                [0; 32]
            },
            representation_authority: representation_authority.to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            realm: if terminal { realm_id } else { [0; 32] },
            collateral_recipient: recipient.map_or([0; 32], |value| value.to_bytes()),
            expected_representation_revision: representation_revision,
            expected_claims_market_revision: match action {
                RepresentationActionV2::Denominate => 0,
                RepresentationActionV2::Reconstitute => 1,
                RepresentationActionV2::RedeemTerminal => 0,
                RepresentationActionV2::IssueStructured
                | RepresentationActionV2::UnwrapStructured => ABSENT_REVISION,
            },
            expected_actor_position_revision: match action {
                RepresentationActionV2::Denominate => 0,
                RepresentationActionV2::Reconstitute => 1,
                RepresentationActionV2::IssueStructured
                | RepresentationActionV2::UnwrapStructured
                | RepresentationActionV2::RedeemTerminal => ABSENT_REVISION,
            },
            expected_custody_position_revision: match action {
                RepresentationActionV2::Denominate => 0,
                RepresentationActionV2::Reconstitute => 1,
                RepresentationActionV2::RedeemTerminal => 0,
                RepresentationActionV2::IssueStructured
                | RepresentationActionV2::UnwrapStructured => ABSENT_REVISION,
            },
            expected_custody_replay_revision: if terminal && selected_outcome == WINNER {
                CUSTODY_EXPECTED_REVISION
            } else {
                ABSENT_REVISION
            },
            generation: GENERATION,
            quantity: 1,
            denominator: DENOMINATOR,
            expected_receipt_supply: receipt_supply,
            outcome_count: OUTCOME_COUNT,
            selected_outcome: selected,
            asset_count,
        },
        &rows,
    )
    .expect("canonical representation request");
    let mut output = vec![0; request.wire_len().expect("class width")];
    request
        .encode_into(&mut output)
        .expect("encode representation request");
    output
}

fn request_bytes(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
) -> Vec<u8> {
    request_bytes_for_selected(fixture, action, representation_revision, WINNER)
}

fn request_bytes_for_selected(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
    selected_outcome: u32,
) -> Vec<u8> {
    let issued = representation_revision == 1 && action == RepresentationActionV2::UnwrapStructured;
    let denominated = action == RepresentationActionV2::Reconstitute;
    request_bytes_from(
        action,
        fixture.release_set,
        fixture.market,
        fixture.graph_id,
        fixture.descriptor_id,
        fixture.parent_context,
        fixture.actor.pubkey(),
        fixture.receipt_mint,
        fixture.actor_receipt,
        fixture.representation_authority,
        fixture.realm_id,
        fixture.terminal_accounts.map(|value| value.recipient),
        representation_revision,
        if issued {
            RECEIPT_SUPPLY + 1
        } else {
            RECEIPT_SUPPLY
        },
        if issued {
            actor_shards_after_issue()
        } else if denominated {
            actor_shards_after_denominate()
        } else {
            actor_shards()
        },
        if issued {
            structured_shards_after_issue()
        } else {
            structured_shards()
        },
        fixture.assets,
        selected_outcome,
    )
}

fn outer_caller_authority(request_bytes: &[u8], market: Pubkey, release_set: [u8; 32]) -> Pubkey {
    let request = RepresentationRequestV2::decode(request_bytes).expect("canonical request");
    let header = request.header();
    assert_eq!(header.market, market.to_bytes());
    assert_eq!(header.release_set, release_set);
    caller_authority_for_digest(request_bytes, market, release_set, header.parent_context)
}

/// The Trading caller authority for request bytes the canonical decoder REFUSES.
///
/// The wrapper derives its signing PDA from the request digest without ever
/// decoding the request, so a hostile whose bytes the grammar rejects still has
/// a well-defined authority and can still be submitted. Without this, a hostile
/// the request grammar catches could never be driven to the chain at all, and
/// the campaign would be asserting the host decoder against itself.
fn caller_authority_for_digest(
    request_bytes: &[u8],
    market: Pubkey,
    release_set: [u8; 32],
    parent_context: [u8; 32],
) -> Pubkey {
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release_set).expect("release set"),
        market.to_bytes(),
        ExecutionRoleV1::Trading,
        parent_context,
        hash(request_bytes).to_bytes(),
    )
    .expect("Trading caller seeds");
    Pubkey::find_program_address(&seeds.as_slices(), &TEST_CALLER_PROGRAM_ID).0
}

#[allow(clippy::too_many_arguments)]
fn terminal_custody(
    request_bytes: &[u8],
    release_set: [u8; 32],
    market: Pubkey,
    realm_id: [u8; 32],
    custody_context: [u8; 32],
    actor: Pubkey,
    collateral_mint: Pubkey,
    recipient: Pubkey,
    candidate_digest: [u8; 32],
    amount: u64,
) -> (CustodyRequestV1, Pubkey, Pubkey, Pubkey, Pubkey) {
    let mut request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CustodyCallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set,
        market: market.to_bytes(),
        realm: realm_id,
        context: custody_context,
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: [0; 32],
            destination_owner: actor.to_bytes(),
            order: [0; 32],
            parent_request_digest: hash(request_bytes).to_bytes(),
            order_nonce: 0,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [0x91; 32],
        destination: recipient.to_bytes(),
        // The Hoard Vault lives under the Market's Custody namespace, which is
        // where the founding put it. Writing `market.to_bytes()` here — as this
        // helper did until this campaign — would bend the fixture to the side of
        // the guard under test and hide the defect it exists to witness.
        source_vault_context: custody_context,
        destination_vault_context: [0; 32],
        mint: collateral_mint.to_bytes(),
        token_program: TOKEN_2022_PROGRAM_ID,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: CUSTODY_EXPECTED_REVISION,
        resulting_revision: CUSTODY_EXPECTED_REVISION + 1,
        amount,
        rent_lamports: 0,
    };
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::from_request(request, true).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    request.source = hoard.to_bytes();
    let request_bytes = request.to_bytes().expect("canonical Custody request");
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(release_set).expect("release set"),
        market.to_bytes(),
        ExecutionRoleV1::Claims,
        custody_context,
        hash(&request_bytes).to_bytes(),
    )
    .expect("Claims caller seeds");
    let caller = Pubkey::find_program_address(&caller_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
    (request, caller, replay, hoard, custody_authority)
}

#[allow(clippy::too_many_arguments)]
fn terminal_candidate_digest(
    request_bytes: &[u8],
    market: Pubkey,
    aggregate: Pubkey,
    release_set: [u8; 32],
    realm_id: [u8; 32],
    receipt_mint: Pubkey,
    representation_authority: Pubkey,
    descriptor_id: [u8; 32],
    descriptor_bytes: &[u8],
    graph_bytes: &[u8],
    graph_digest: [u8; 32],
    product: &ProductClaimsFixture,
    selected: AssetFixture,
    terminal: TerminalScenarioV3,
    hoard_before: u64,
) -> [u8; 32] {
    let admission = admit_product_representation_v3(ProductRepresentationInputV3 {
        product_basis_bytes: &product.linked_basis_bytes,
        product: ProductRuntimeProjectionV3 {
            product_id: product.product_id,
            result_domain_id: product.result_domain_id,
            coordinate_domain_id: [0x62; 32],
            result_unit_id: [0x63; 32],
            semantic_basis_id: product.basis_id,
            linked_basis_record_digest: product.linked_basis_digest,
            evaluator_release_id: [0x68; 32],
            basis_width: OUTCOME_COUNT,
            payout_scale: product.payout_scale,
        },
        descriptor_bytes,
        descriptor_admission: DescriptorAdmissionV2 {
            selected_descriptor_id: descriptor_id,
            finalized_descriptor_id: descriptor_id,
            recomputed_descriptor_digest: descriptor_id,
            finalized_descriptor_digest: descriptor_id,
            record_authenticated: true,
            derived_representation_authority: representation_authority.to_bytes(),
            authority_derivation_authenticated: true,
        },
        graph_bytes,
        graph_admission: ContentAdmissionV2 {
            selected_graph_id: EXPOSURE_ID,
            finalized_graph_id: EXPOSURE_ID,
            recomputed_graph_digest: graph_digest,
            finalized_graph_digest: graph_digest,
            record_authenticated: true,
        },
        context: RepresentationContextV3 {
            market_id: market.to_bytes(),
            release_set_id: release_set,
            claims_basis_id: product.basis_id,
            claims_width: OUTCOME_COUNT,
            receipt_mint: receipt_mint.to_bytes(),
            token_program: TOKEN_2022_PROGRAM_ID,
            representation_authority: representation_authority.to_bytes(),
        },
    })
    .expect("Product/representation terminal admission")
    .admission();
    let market_bytes = encode_liability_basis_market_v2(
        LiabilityBasisMarketInputV2 {
            revision: 0,
            logical_market: market.to_bytes(),
            release_set,
            registry_program: REGISTRY_PROGRAM_ID.to_bytes(),
            product_instance_id: product.product_id,
            basis_id: product.basis_id,
            realm_id,
            custody_context: founding_custody_context(),
            generation: GENERATION,
        },
        &aggregate_claims(),
    )
    .expect("terminal aggregate prestate");
    let position_bytes = encode_liability_basis_position_v2(
        LiabilityBasisPositionInputV2 {
            revision: 0,
            market_account: aggregate.to_bytes(),
            owner: selected.custody_owner.to_bytes(),
            basis_id: product.basis_id,
        },
        &custody_claims(usize::try_from(WINNER).expect("winner index")),
    )
    .expect("terminal Position prestate");
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral delta");
    let mut payouts = vec![0_u64; usize::try_from(OUTCOME_COUNT).expect("outcome width")];
    let mut translation_scratch = vec![0_u64; payouts.len()];
    let mut claims_payouts = vec![0_u64; payouts.len()];
    let mut aggregate_deltas = vec![neutral; payouts.len()];
    let mut packet = vec![0_u8; plan_bytes(OUTCOME_COUNT, 1, 1).expect("terminal packet width")];
    let payout = encode_product_basis_terminal_signed_delta_v3(
        ProductBasisTerminalInputV3 {
            product_basis_bytes: &product.linked_basis_bytes,
            representation: admission,
            composition_exposure_bytes: graph_bytes,
            composition_exposure_admission: RecordAdmissionV3 {
                selected_id: EXPOSURE_ID,
                finalized_id: EXPOSURE_ID,
                recomputed_digest: graph_digest,
                finalized_digest: graph_digest,
                record_authenticated: true,
            },
            product_record_digest: product.product_digest,
            market_account: aggregate.to_bytes(),
            market_bytes: &market_bytes,
            position_bytes: &position_bytes,
            owner: selected.custody_owner.to_bytes(),
            request_id: hash(request_bytes).to_bytes(),
            caller_role: CallerRole::Trading,
            terminal,
            claim_index: WINNER,
            quantity: 1,
            expected_generation: GENERATION,
            expected_market_revision: 0,
            expected_position_revision: 0,
            hoard_before,
        },
        &mut payouts,
        &mut translation_scratch,
        &mut claims_payouts,
        &mut aggregate_deltas,
        &mut packet,
    )
    .expect("terminal ProductBasis plan");
    assert!(payout > 0, "the staged representation redemption must pay");
    hashv(&[
        TERMINAL_CANDIDATE_DOMAIN_V3,
        &hash(&packet).to_bytes(),
        &payout.to_le_bytes(),
        &admission.to_bytes(),
        &WINNER.to_le_bytes(),
    ])
    .to_bytes()
}

/// The transfer a freshly created Claims-role replay must admit.
///
/// `InitializeReplay` writes a cursor at revision one with zero open Vaults, and
/// the redemption's Custody request consumes exactly that. Asserted against the
/// contract rather than against the chain so a campaign whose numbers drifted
/// fails while the fixture is being built, not four transactions later inside a
/// rolled-back instruction.
fn replay_admissible_from_creation(transfer: CustodyRequestV1, payer: Pubkey) {
    let creation = CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CustodyCallerRoleV1::Claims,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: transfer.release_set,
        market: transfer.market,
        realm: transfer.realm,
        context: transfer.context,
        caller_program: transfer.caller_program,
        semantic: ContextV1 {
            candidate: [0; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: [0; 32],
            parent_request_digest: [0x92; 32],
            order_nonce: 0,
            generation: transfer.semantic.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer: payer.to_bytes(),
        rent_refund: payer.to_bytes(),
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        rent_lamports: 1,
    };
    let created = CustodyReplayV1::initialize(creation, [0x92; 32], [0x93; 32])
        .expect("InitializeReplay mints the Claims-role replay");
    assert_eq!(created.next_revision, CUSTODY_EXPECTED_REVISION);
    assert_eq!(created.open_vault_count, 0);
    created
        .advance(
            transfer,
            hash(&transfer.to_bytes().expect("Custody request")).to_bytes(),
            [0x94; 32],
        )
        .expect("a created Claims-role replay admits the terminal transfer");
}

/// Which terminal a fixture stages, if any.
///
/// This used to be a `bool`, which could only ever express the terminal a
/// PROVIDER stood behind. A market can also end because nobody resolved it: a
/// third party walks it past its own deadline, collects the pre-funded bounty,
/// and Core admits a `ResolutionFailure` certificate at the Product's
/// pre-disclosed failure region. Both are terminals a holder must be able to
/// exit through, and only one of them had ever been staged anywhere.
#[derive(Clone, Copy, Eq, PartialEq)]
enum TerminalV1 {
    /// An open Market with no terminal at all.
    None,
    /// A provider-backed resolution at [`WINNER`].
    Provider,
    /// The market's own pre-disclosed failure terms at [`FAILURE_SELECTOR`].
    Failure,
    /// A deliberately mismatched terminal, for the hostiles that pin the seam
    /// between which coordinate Core commits and which kind Resolution wrote.
    Mismatched {
        /// The coordinate Core commits as the winner.
        winner: u32,
        /// The certificate kind Resolution wrote for it.
        kind: ResolutionCertificateKindV2,
    },
}

/// The ProductBasisV3 family one fixture actually places behind the finalized
/// Registry account.
///
/// Existing representation tests keep their categorical profile. The curved
/// degree-two profile drives the curved evaluator through the complete
/// Structured/Rational route. Degree three needs four control points and is
/// exercised by the Fractional campaign, without pretending this child wire's
/// K=3 ceiling can carry it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProductBasisProfileV1 {
    Categorical,
    CurvedDegreeTwo,
}

impl ProductBasisProfileV1 {
    const fn kind(self) -> BasisKindV3 {
        match self {
            Self::Categorical => BasisKindV3::CategoricalQ1,
            Self::CurvedDegreeTwo => BasisKindV3::SplineDegree2To3 {
                degree: 2,
                interior_multiplicity: false,
            },
        }
    }

    const fn spline_degree(self) -> Option<u8> {
        match self {
            Self::Categorical => None,
            Self::CurvedDegreeTwo => Some(2),
        }
    }

    fn knots(self) -> Vec<i128> {
        match self {
            Self::Categorical => Vec::new(),
            // One clamped quadratic Bernstein span over [0,3].
            Self::CurvedDegreeTwo => vec![0, 0, 0, 3, 3, 3],
        }
    }

    fn failure_payouts(self) -> Vec<u64> {
        match self {
            Self::Categorical => Vec::new(),
            Self::CurvedDegreeTwo => vec![0, 0, self.payout_scale()],
        }
    }

    const fn knot_denominator(self) -> u64 {
        match self {
            Self::Categorical | Self::CurvedDegreeTwo => 1,
        }
    }

    const fn expected_curve_payouts(self) -> [u64; K] {
        match self {
            Self::CurvedDegreeTwo => [1, 4, 2],
            Self::Categorical => [0; K],
        }
    }

    const fn payout_scale(self) -> u64 {
        match self {
            Self::Categorical => 1,
            Self::CurvedDegreeTwo => CURVED_PAYOUT_SCALE,
        }
    }

    fn terminal_scenario(self, terminal: TerminalV1) -> TerminalScenarioV3 {
        match (self, terminal.certificate_kind()) {
            (Self::Categorical, _) => TerminalScenarioV3::Categorical(terminal.winner()),
            (Self::CurvedDegreeTwo, ResolutionCertificateKindV2::ResolutionSuccess) => {
                TerminalScenarioV3::Rational {
                    numerator: CURVED_RESULT_NUMERATOR,
                    denominator: CURVED_RESULT_DENOMINATOR,
                }
            }
            (Self::CurvedDegreeTwo, ResolutionCertificateKindV2::ResolutionFailure) => {
                TerminalScenarioV3::Failure
            }
            // These are not terminal certificates. Refuse to invent a Product
            // interpretation if a future fixture tries to place one here.
            (
                Self::CurvedDegreeTwo,
                ResolutionCertificateKindV2::RecoveryAdvanced
                | ResolutionCertificateKindV2::Exhausted,
            ) => panic!("a non-terminal certificate has no payoff coordinate"),
        }
    }

    const fn certificate_result(self, terminal: TerminalV1) -> (i128, u64) {
        match terminal.certificate_kind() {
            ResolutionCertificateKindV2::ResolutionFailure => (0, 0),
            ResolutionCertificateKindV2::ResolutionSuccess => match self {
                Self::Categorical => (1, 1),
                Self::CurvedDegreeTwo => (CURVED_RESULT_NUMERATOR, CURVED_RESULT_DENOMINATOR),
            },
            ResolutionCertificateKindV2::RecoveryAdvanced
            | ResolutionCertificateKindV2::Exhausted => (0, 0),
        }
    }
}

impl TerminalV1 {
    /// Whether the Market carries a terminal receipt at all.
    const fn resolved(self) -> bool {
        !matches!(self, Self::None)
    }

    /// The coordinate Core commits as the winner.
    ///
    /// A failure terminal is not a flag on an ordinary one: the failure region
    /// is a COORDINATE, and `validate_terminal_product` admits a
    /// `ResolutionFailure` at exactly the Product's final one and nowhere else.
    const fn winner(self) -> u32 {
        match self {
            Self::Failure => FAILURE_SELECTOR,
            Self::None | Self::Provider => WINNER,
            Self::Mismatched { winner, .. } => winner,
        }
    }

    /// The exact Lean-owned certificate kind Resolution wrote.
    const fn certificate_kind(self) -> ResolutionCertificateKindV2 {
        match self {
            Self::Failure => ResolutionCertificateKindV2::ResolutionFailure,
            Self::None | Self::Provider => ResolutionCertificateKindV2::ResolutionSuccess,
            Self::Mismatched { kind, .. } => kind,
        }
    }
}

fn fixture(terminal: bool) -> (ProgramTest, Fixture) {
    fixture_with(
        if terminal {
            TerminalV1::Provider
        } else {
            TerminalV1::None
        },
        ReceiptMintRoles::Both,
    )
}

fn fixture_with(terminal: TerminalV1, receipt_roles: ReceiptMintRoles) -> (ProgramTest, Fixture) {
    fixture_with_basis(terminal, receipt_roles, ProductBasisProfileV1::Categorical)
}

/// The same resolved fixture, on a bank where the TRADING ROLE IS TRADING.
///
/// `fixture` activates the rational-v2 TEST CALLER in the Trading role, because
/// the representation route drives that caller as a Trading-role caller and its
/// signing PDA is derived under it (`caller_authority_for_digest`). That is
/// sound for the route it serves and it is a release set no cohort has -- the
/// three-hats substitution `5dc77408` removed from the Resolution campaign.
///
/// A wallet payout is `CallerRole::Claims` and never enters the Trading role
/// (`signed_delta_v3.rs`: the Trading projection is requested only when
/// `caller_coordinate` is `Caller`), so it can be proved on the release set a
/// cohort actually runs. Nothing is re-pinned to get there: the release-set id
/// moves, and with it the Market, the aggregate, the Position, the Hoard, the
/// Custody replay and the certificate -- every one of which this harness
/// DERIVES. The one thing that would have gone red is a pin to a document, and
/// the payout has none.
fn fixture_with_real_trading_role(terminal: TerminalV1) -> (ProgramTest, Fixture) {
    fixture_with_basis_and_trading(
        terminal,
        ReceiptMintRoles::Both,
        ProductBasisProfileV1::Categorical,
        Some(trading_artifact()),
        CollateralAdapterSelectionV1::Cohort13ZeroExtension,
    )
}

/// Which collateral adapter release this fixture's Realm is founded under.
///
/// A realm record on chain stores the SHA-256 of a `CollateralAdapterReleaseV1`
/// preimage as `collateral_adapter_release_id`, and Custody selects a profile by
/// matching it. That id is therefore not a fixture detail: it is the whole
/// difference between a market that can pay a wallet its own associated token
/// account and one that cannot, and it is fixed for the life of the market at
/// founding. Both arms are exercised in this file, on the same real ELFs, with
/// the same 170-byte destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollateralAdapterSelectionV1 {
    /// `228c14f9...` -- Token-2022 at exact base widths. Cohort-13 founded here.
    Cohort13ZeroExtension,
    /// `430369ce...` -- the same interface, admitting the ATA program's
    /// `ImmutableOwner` on transfer participants. Cohort-14 founds here.
    Cohort14ImmutableOwner,
}

impl CollateralAdapterSelectionV1 {
    fn release(self) -> CollateralAdapterReleaseV1 {
        match self {
            Self::Cohort13ZeroExtension => {
                CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer()
            }
            Self::Cohort14ImmutableOwner => {
                CollateralAdapterReleaseV1::token_2022_immutable_owner_exact_transfer()
            }
        }
    }
}

/// The same resolved real-Trading fixture, founded under cohort-14's release.
fn fixture_on_the_immutable_owner_release(terminal: TerminalV1) -> (ProgramTest, Fixture) {
    fixture_with_basis_and_trading(
        terminal,
        ReceiptMintRoles::Both,
        ProductBasisProfileV1::Categorical,
        Some(trading_artifact()),
        CollateralAdapterSelectionV1::Cohort14ImmutableOwner,
    )
}

fn fixture_with_basis(
    terminal: TerminalV1,
    receipt_roles: ReceiptMintRoles,
    basis_profile: ProductBasisProfileV1,
) -> (ProgramTest, Fixture) {
    fixture_with_basis_and_trading(
        terminal,
        receipt_roles,
        basis_profile,
        None,
        CollateralAdapterSelectionV1::Cohort13ZeroExtension,
    )
}

fn fixture_with_basis_and_trading(
    terminal: TerminalV1,
    receipt_roles: ReceiptMintRoles,
    basis_profile: ProductBasisProfileV1,
    real_trading_elf: Option<Vec<u8>>,
    collateral_adapter: CollateralAdapterSelectionV1,
) -> (ProgramTest, Fixture) {
    let artifacts = artifacts();
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    // NO FORCED BUDGET. `set_compute_max_units` installs a FIXED
    // `RuntimeConfig.compute_budget` for every transaction -- `heap_size`
    // included, at `ComputeBudget::new_with_defaults`' 32 KiB -- after which the
    // per-transaction `RequestHeapFrame` is never consulted
    // (`solana-program-test-4.3.0-beta.2/src/lib.rs:1074`). This campaign drives
    // `DCLTHOT3`, which declares an extended heap and whose adapter sizes its
    // scratch ceiling from the request it reads in the instructions sysvar. A
    // forced budget therefore let the program believe it had 64 KiB while the VM
    // mapped 32 KiB, and its first scratch write landed outside the mapping as an
    // ACCESS VIOLATION rather than as any refusal. `direct_hot_top_level` avoids
    // exactly this with `program_test_without_forced_budget`.
    //
    // So the limit is asked for ON THE WIRE, which is what a real caller does and
    // what makes the bank derive the whole budget per transaction.
    for (name, program, elf) in [
        (
            "dclutch_claims_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.claims.as_slice(),
        ),
        (
            "dclutch_custody_sbf",
            CUSTODY_PROGRAM_ID,
            artifacts.custody.as_slice(),
        ),
        (
            "dclutch_registry_sbf",
            REGISTRY_PROGRAM_ID,
            artifacts.registry.as_slice(),
        ),
        (
            "dclutch_core_sbf",
            CORE_PROGRAM_ID,
            artifacts.core.as_slice(),
        ),
        (
            "dclutch_resolution_proof_sbf",
            RESOLUTION_PROGRAM_ID,
            artifacts.resolution.as_slice(),
        ),
        (
            "dclutch_rational_v2_test_caller_sbf",
            TEST_CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
        (
            "spl_token_2022",
            TOKEN_PROGRAM_ID,
            artifacts.token_2022.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }
    if let Some(trading) = real_trading_elf.as_deref() {
        add_upgradeable_program(
            &mut test,
            "dclutch_trading_sbf",
            TRADING_PROGRAM_ID,
            trading,
        );
    }

    let actor = Keypair::new_from_array(if terminal.resolved() {
        [0x72; 32]
    } else {
        [0x71; 32]
    });
    // The actor PREPAYS the Claims-role Custody replay in the terminal fixture,
    // so it needs the replay's rent on top of its own rent exemption. Sized from
    // the replay width rather than a round number: a fixture constant chosen to
    // make a transfer succeed is a fixture deciding a protocol fact.
    // Enough for the replay rent the actor prepays in step one AND for the fees
    // of step two, which the wallet pays for itself in
    // `the_position_owner_pays_its_own_fee_and_still_authorizes_the_payout`. The
    // width is doubled rather than a lamport literal added so it stays a rent
    // computation and not a magic number.
    add_funded_empty(
        &mut test,
        actor.pubkey(),
        CUSTODY_REPLAY_BYTES_V1
            .checked_mul(2)
            .expect("actor funding width"),
    );
    let (release_set, cache_data) = match real_trading_elf.as_deref() {
        Some(trading) => activation_cache_for_upstream(&artifacts, TRADING_PROGRAM_ID, trading),
        None => activation_cache(&artifacts),
    };
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(&mut test, activation_cache, REGISTRY_PROGRAM_ID, cache_data);

    let collateral_mint = Pubkey::new_from_array(if terminal.resolved() {
        [0x74; 32]
    } else {
        [0x73; 32]
    });
    let adapter = collateral_adapter.release();
    assert!(
        PRODUCTION_ADAPTER_RELEASES.contains(&adapter),
        "a fixture Realm must select a release the programs can find in the catalog",
    );
    let realm = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: collateral_mint.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let realm_bytes = realm.to_bytes();
    let (realm_raw, realm_staging, realm_id) =
        add_finalized_record(&mut test, REALM_SCHEMA_RELEASE_ID_V1, &realm_bytes);

    let product_claims = add_product_claims(&mut test, basis_profile);
    let hot_release = real_trading_elf.as_ref().map(|_| {
        common_hot_open::compile_release(realm_id, release_set, &product_claims.linked_basis_bytes)
    });
    let terminal_scenario = basis_profile.terminal_scenario(terminal);
    let mut terminal_payouts = [0_u64; K];
    if terminal.resolved() {
        let basis = ProductBasisV3::decode(&product_claims.linked_basis_bytes)
            .expect("fixture ProductBasisV3");
        match terminal_scenario {
            TerminalScenarioV3::Categorical(selector) => basis
                .evaluate_categorical(selector, &mut terminal_payouts)
                .expect("categorical terminal partition"),
            TerminalScenarioV3::Rational {
                numerator,
                denominator,
            } => basis
                .evaluate_rational(numerator, denominator, &mut terminal_payouts)
                .expect("rational terminal partition"),
            TerminalScenarioV3::Failure => basis
                .evaluate_failure(&mut terminal_payouts)
                .expect("failure terminal partition"),
        }
    }
    let initial_hoard_atoms = if terminal.resolved() {
        aggregate_claims()
            .iter()
            .zip(terminal_payouts)
            .try_fold(0_u64, |total, (supply, payout)| {
                total.checked_add(supply.checked_mul(payout)?)
            })
            .expect("terminal liability fits u64")
    } else {
        INITIAL_HOARD_ATOMS
    };
    if basis_profile == ProductBasisProfileV1::Categorical && terminal.resolved() {
        assert_eq!(initial_hoard_atoms, INITIAL_HOARD_ATOMS);
    }
    let terminal_certificate = terminal
        .resolved()
        .then(|| Pubkey::new_from_array([0x86; 32]));
    let (market, core_data) = core_market(
        release_set,
        realm_id,
        product_claims.product_digest,
        product_claims.product_id,
        terminal_certificate.map(|certificate| certificate.to_bytes()),
        terminal.winner(),
        hot_release.as_ref().map_or([0x64; 32], |release| {
            hash(&release.manifest.bytes).to_bytes()
        }),
    );
    add_account(&mut test, market, CORE_PROGRAM_ID, core_data);
    add_account(
        &mut test,
        market_rent_credit(),
        system_program::ID,
        Vec::new(),
    );
    if let Some(certificate) = terminal_certificate {
        // Nothing that differs between the two kinds is a choice this campaign
        // made. `validate_shape` forces every one of them: a provider-backed
        // success must carry a route, provider evidence and a result; the
        // market's own failure terms must carry NONE of those, must carry the
        // funding allocation the walk consumed, and must carry nonzero work
        // paid -- the bounty a third party collected for finishing a market its
        // relayer abandoned. `to_bytes` refuses any other combination.
        let failed = matches!(
            terminal.certificate_kind(),
            ResolutionCertificateKindV2::ResolutionFailure
        );
        let (result_numerator, result_denominator) = basis_profile.certificate_result(terminal);
        let bytes = ResolutionCertificateV2 {
            kind: terminal.certificate_kind(),
            market: market.to_bytes(),
            route: if failed { [0; 32] } else { [0x87; 32] },
            source_material: [0x63; 32],
            product_record_digest: product_claims.product_digest,
            provider_evidence: if failed { [0; 32] } else { [0x88; 32] },
            funding_allocation: if failed { [0x63; 32] } else { [0; 32] },
            receipt_account: certificate.to_bytes(),
            generation: GENERATION,
            attempt_index: 0,
            schedule_index: 0,
            selector: terminal.winner(),
            work_paid: if failed {
                FAILURE_WALK_BOUNTY_LAMPORTS
            } else {
                0
            },
            funding_remaining: 0,
            result_numerator,
            result_denominator,
            observed_at: u64::from(!failed),
        }
        .to_bytes()
        .expect("canonical Resolution certificate");
        add_account(
            &mut test,
            certificate,
            RESOLUTION_PROGRAM_ID,
            bytes.to_vec(),
        );
    }
    let aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let receipt_mint = Pubkey::new_from_array(if terminal.resolved() {
        [0x76; 32]
    } else {
        [0x75; 32]
    });
    // The actor's receipt holding is an Associated Token Account, not a tag.
    //
    // The receipt Mint above stays a written value because the descriptor
    // *names* it and every reader takes it from there. This account is the
    // opposite kind of coordinate: nothing names it, so `authenticate_actor_receipt`
    // derives it, and a builder handed some other address has been handed an
    // account the wallet it builds for could never be holding. The two branches
    // stay distinct without being typed, because the resolved and unresolved
    // campaigns already run under different actors and different receipt Mints.
    let actor_receipt = get_associated_token_address_with_program_id(
        &actor.pubkey(),
        &receipt_mint,
        &TOKEN_PROGRAM_ID,
    );
    // THE DESCRIPTOR IS DERIVED, not written. `structured_lowering::lower`
    // builds one canonical composition (graph, translation, composition
    // descriptor), the exposure record the chain will hold, the shard layer and
    // the immutable Structured terms, then hands all three to
    // `derive_structured_representation_descriptor_v2`, whose `descriptor_id`
    // is the digest of the preimage it wrote. Before this campaign the preimage
    // was hand-filled here — the sixth such producer in the tree, and the only
    // one that ever reached a real ELF.
    let basis = campaign_basis(market, release_set, receipt_mint, &product_claims);
    let lowering = structured_lowering::lower(&basis);
    let graph = lowering.exposure.clone();
    let (graph_raw, graph_staging, graph_digest) =
        add_finalized_record(&mut test, COMPOSITION_EXPOSURE_SCHEMA_ID_V3, &graph);
    // The same-width exposure hostile: coordinate 0 weighs its Product
    // coordinate twice. Still a canonical record, still `K` rows, different
    // bytes.
    let alternate_graph = structured_lowering::exposure_bytes(
        &basis,
        &std::array::from_fn::<u64, K, _>(|index| if index == 0 { 2 } else { 1 }),
    );
    let (alternate_graph_raw, alternate_graph_staging, alternate_graph_digest) =
        add_finalized_record(
            &mut test,
            COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
            &alternate_graph,
        );
    assert_ne!(graph_digest, alternate_graph_digest);
    let descriptor = lowering.descriptor.preimage.clone();
    let (descriptor_raw, descriptor_staging, descriptor_id) = add_finalized_record(
        &mut test,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        &descriptor,
    );
    assert_eq!(
        descriptor_id, lowering.descriptor.descriptor_id,
        "the finalized record's digest IS the derived descriptor identity"
    );
    // The recipe this composition does NOT state: the canonical coefficients at
    // the wrong coordinates. Refused TWICE, and the two refusals are different
    // facts. Host-side, `require_coefficients_are_the_composition_root` refuses
    // to MINT it — the join the live chain route lost when
    // `authenticate_exposure` replaced `authenticate_graph` (decision 0011 §3d),
    // and the reason founding is the last moment a recipe can be checked.
    // On-chain, the substituted ACCOUNT below is refused because the request
    // names a `descriptor_id` that is the digest of other bytes.
    assert_eq!(
        structured_lowering::lower_against_root(&basis, &PERMUTED_COEFFICIENTS).err(),
        Some(StructuredOperatorError::Terms),
        "the derivation must refuse coefficients that are not the composition root"
    );
    let alternate_descriptor =
        descriptor_with_substituted_coefficients(&descriptor, &PERMUTED_COEFFICIENTS);
    let (alternate_descriptor_raw, alternate_descriptor_staging, alternate_descriptor_id) =
        add_finalized_record(
            &mut test,
            REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
            &alternate_descriptor,
        );
    assert_ne!(descriptor_id, alternate_descriptor_id);

    let representation_authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        representation_authority,
        system_program::ID,
        Vec::new(),
    );
    let assets = std::array::from_fn(|index| {
        let outcome = u32::try_from(index).expect("outcome");
        let outcome_bytes = outcome.to_le_bytes();
        let mint = Pubkey::find_program_address(
            &[RATIONAL_SHARD_MINT_SEED_V2, &descriptor_id, &outcome_bytes],
            &CLAIMS_PROGRAM_ID,
        )
        .0;
        let custody_owner_seeds =
            ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor_id, outcome)
                .expect("Claims capability owner seeds");
        let custody_owner =
            Pubkey::find_program_address(&custody_owner_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
        let position_seeds =
            ProtocolPositionSeedsV2::new(aggregate.to_bytes(), custody_owner.to_bytes())
                .expect("custody Position seeds");
        let position =
            Pubkey::find_program_address(&position_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
        // Derived for the same reason as `actor_receipt` above, and the reason
        // is visible in this very block: the Mint, the custody owner, the
        // Position and the Structured custody below are all canonical
        // derivations that agree with the operator. This one coordinate was a
        // typed tag, so it was the one address in the campaign that no rule
        // produces.
        let actor_token =
            get_associated_token_address_with_program_id(&actor.pubkey(), &mint, &TOKEN_PROGRAM_ID);
        let structured_token = Pubkey::find_program_address(
            &[
                RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
                &descriptor_id,
                &outcome_bytes,
            ],
            &CLAIMS_PROGRAM_ID,
        )
        .0;
        let obsolete_structured_ata = get_associated_token_address_with_program_id(
            &representation_authority,
            &mint,
            &TOKEN_PROGRAM_ID,
        );
        AssetFixture {
            custody_owner,
            position,
            mint,
            actor_token,
            structured_token,
            obsolete_structured_ata,
        }
    });

    let actor_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), actor.pubkey().to_bytes())
            .expect("actor Position seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let market_input = LiabilityBasisMarketInputV2 {
        revision: 0,
        logical_market: market.to_bytes(),
        release_set,
        registry_program: REGISTRY_PROGRAM_ID.to_bytes(),
        product_instance_id: product_claims.product_id,
        basis_id: product_claims.basis_id,
        realm_id,
        // What `FoundingV5` persists for a `DCLTGMF1`-founded Market: the
        // authenticated Custody namespace the realized replay carries.
        custody_context: founding_custody_context(),
        generation: GENERATION,
    };
    let aggregate_data = encode_liability_basis_market_v2(market_input, &aggregate_claims())
        .expect("LBV2 Claims aggregate");
    add_account(&mut test, aggregate, CLAIMS_PROGRAM_ID, aggregate_data);
    add_account(
        &mut test,
        actor_position,
        CLAIMS_PROGRAM_ID,
        encode_liability_basis_position_v2(
            LiabilityBasisPositionInputV2 {
                revision: 0,
                market_account: aggregate.to_bytes(),
                owner: actor.pubkey().to_bytes(),
                basis_id: product_claims.basis_id,
            },
            &ACTOR_CLAIMS,
        )
        .expect("actor Position"),
    );
    for (index, asset) in assets.iter().enumerate() {
        let claims = custody_claims(index);
        add_account(
            &mut test,
            asset.position,
            CLAIMS_PROGRAM_ID,
            encode_liability_basis_position_v2(
                LiabilityBasisPositionInputV2 {
                    revision: 0,
                    market_account: aggregate.to_bytes(),
                    owner: asset.custody_owner.to_bytes(),
                    basis_id: product_claims.basis_id,
                },
                &claims,
            )
            .expect("custody Position"),
        );
        add_account(
            &mut test,
            asset.mint,
            TOKEN_PROGRAM_ID,
            claim_mint_data(
                representation_authority,
                shard_supply(index),
                *SHARD_DECIMALS.get(index).expect("shard decimals"),
            ),
        );
        add_account(
            &mut test,
            asset.actor_token,
            TOKEN_PROGRAM_ID,
            token_account_data(
                asset.mint,
                actor.pubkey(),
                *actor_shards().get(index).expect("actor shards"),
            ),
        );
        add_account(
            &mut test,
            asset.structured_token,
            TOKEN_PROGRAM_ID,
            token_account_data(
                asset.mint,
                representation_authority,
                *structured_shards().get(index).expect("structured shards"),
            ),
        );
        add_account(
            &mut test,
            asset.obsolete_structured_ata,
            TOKEN_PROGRAM_ID,
            token_account_data(
                asset.mint,
                representation_authority,
                *structured_shards()
                    .get(index)
                    .expect("obsolete structured shards"),
            ),
        );
    }
    add_account(
        &mut test,
        receipt_mint,
        TOKEN_PROGRAM_ID,
        mint_with_roles(
            representation_authority,
            RECEIPT_SUPPLY,
            RECEIPT_DECIMALS,
            receipt_roles,
        ),
    );
    add_account(
        &mut test,
        actor_receipt,
        TOKEN_PROGRAM_ID,
        token_account_data(receipt_mint, actor.pubkey(), 0),
    );
    let representation_replay = Pubkey::find_program_address(
        &[
            RATIONAL_REPLAY_SEED_V2,
            descriptor_id.as_slice(),
            actor.pubkey().as_ref(),
        ],
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    add_funded_empty(&mut test, representation_replay, RATIONAL_REPLAY_BYTES_V2);

    let parent_context = [0x68; 32];
    let custody_context = founding_custody_context();
    assert_ne!(
        custody_context,
        market.to_bytes(),
        "the Custody namespace must not be the Market address, or this campaign proves nothing"
    );
    assert_ne!(
        custody_context, parent_context,
        "the Custody namespace and the representation parent context are different facts"
    );
    let capability_root = hot_release.as_ref().map(|release| {
        let header = dclutch_capability_program_contract::CapabilityRootHeaderV1::new(
            ContentId::new(release_set).expect("release set"),
            market.to_bytes(),
            GENERATION,
            release.manifest.selection,
            release.manifest.record_bumps,
        )
        .expect("Structured capability root header");
        let (root, root_bump) =
            Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID);
        let mut bytes = header.to_bytes().to_vec();
        let root_tail = dclutch_structured_v2_contract::StructuredRootV2::new(
            dclutch_structured_v2_contract::StructuredRootInputV2 {
                bump: root_bump,
                terms: descriptor_id,
                market: market.to_bytes(),
                rent_beneficiary: market_rent_credit().to_bytes(),
                revision: 0,
                historical_rent_principal: Rent::default().minimum_balance(
                    dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
                        + dclutch_structured_v2_kernel::STRUCTURED_ROOT_BYTES_V2,
                ),
            },
        )
        .expect("Structured root tail");
        bytes.extend_from_slice(&root_tail.to_bytes());
        add_account(&mut test, root, TRADING_PROGRAM_ID, bytes);
        root
    });

    let fixture_stub = Fixture {
        actor,
        basis_profile,
        terminal_scenario,
        initial_hoard_atoms,
        release_set,
        realm_id,
        parent_context,
        custody_context,
        market,
        aggregate,
        actor_position,
        activation_cache,
        claims_programdata: programdata_address(CLAIMS_PROGRAM_ID),
        custody_programdata: programdata_address(CUSTODY_PROGRAM_ID),
        core_programdata: programdata_address(CORE_PROGRAM_ID),
        resolution_programdata: programdata_address(RESOLUTION_PROGRAM_ID),
        caller_programdata: programdata_address(if real_trading_elf.is_some() {
            TRADING_PROGRAM_ID
        } else {
            TEST_CALLER_PROGRAM_ID
        }),
        representation_authority,
        descriptor_id,
        descriptor_raw,
        descriptor_staging,
        alternate_descriptor_raw,
        alternate_descriptor_staging,
        graph_id: EXPOSURE_ID,
        graph_raw,
        graph_staging,
        alternate_graph_raw,
        alternate_graph_staging,
        linked_basis_record: product_claims.linked_basis_record,
        linked_basis_staging: product_claims.linked_basis_staging,
        product_record: product_claims.product_record,
        product_staging: product_claims.product_staging,
        result_domain_record: product_claims.result_domain_record,
        result_domain_staging: product_claims.result_domain_staging,
        portfolio_record: product_claims.portfolio_record,
        portfolio_staging: product_claims.portfolio_staging,
        product_digest: product_claims.product_digest,
        semantic_basis_id: product_claims.basis_id,
        linked_basis_digest: product_claims.linked_basis_digest,
        graph_digest,
        terminal_winner: terminal.winner(),
        terminal_record_digest: terminal_certificate.map(|certificate| certificate.to_bytes()),
        result_domain_digest: product_claims.result_domain_id,
        linked_basis_bytes: product_claims.linked_basis_bytes.clone(),
        graph_bytes: graph.clone(),
        representation_replay,
        receipt_mint,
        actor_receipt,
        assets,
        basis,
        terminal_accounts: None,
        hot_release,
        capability_root,
    };

    let mut fixture = fixture_stub;
    if terminal.resolved() {
        let recipient = Pubkey::new_from_array([0x85; 32]);
        let terminal_request = request_bytes_from(
            RepresentationActionV2::RedeemTerminal,
            fixture.release_set,
            fixture.market,
            fixture.graph_id,
            fixture.descriptor_id,
            fixture.parent_context,
            fixture.actor.pubkey(),
            fixture.receipt_mint,
            fixture.actor_receipt,
            fixture.representation_authority,
            fixture.realm_id,
            Some(recipient),
            0,
            RECEIPT_SUPPLY,
            actor_shards(),
            structured_shards(),
            fixture.assets,
            // The Rational SHARD redemption is a different route from the
            // wallet payout and this campaign never drives it against a failure
            // terminal, so its precompute stays at `WINNER`. What it yields --
            // the Hoard, the Claims-role replay and the Custody authority -- is
            // derived from the Custody namespace and is winner-independent; the
            // one output that is not, the shard route's own caller PDA, is
            // unused by a wallet payout, which derives its whole frame itself.
            WINNER,
        );
        // The representation route is only precomputed to derive its shared
        // Custody namespace. A failure fixture does not execute that route and
        // keeps the historical winning-coordinate control so the request has a
        // nonzero transfer; the wallet path below independently derives the
        // actual failure-coordinate payout from the certificate.
        let (representation_terminal, representation_payout) = match basis_profile {
            // This precomputation belongs to the independent Structured
            // representation route, whose historical control exits the
            // ordinary winning coordinate. A certificate-mismatch fixture
            // must not silently retarget that unrelated route.
            ProductBasisProfileV1::Categorical => (TerminalScenarioV3::Categorical(WINNER), 1),
            ProductBasisProfileV1::CurvedDegreeTwo => (
                terminal_scenario,
                *terminal_payouts.get(WINNERS).expect("winner payout"),
            ),
        };
        let representation_candidate = terminal_candidate_digest(
            &terminal_request,
            fixture.market,
            aggregate,
            fixture.release_set,
            fixture.realm_id,
            fixture.receipt_mint,
            fixture.representation_authority,
            fixture.descriptor_id,
            &descriptor,
            &graph,
            graph_digest,
            &product_claims,
            fixture.assets[1],
            representation_terminal,
            initial_hoard_atoms,
        );
        let (custody_request, custody_caller, custody_replay, hoard, custody_authority) =
            terminal_custody(
                &terminal_request,
                fixture.release_set,
                fixture.market,
                fixture.realm_id,
                fixture.custody_context,
                fixture.actor.pubkey(),
                collateral_mint,
                recipient,
                representation_candidate,
                representation_payout,
            );
        // The Hoard this fixture funds is the one a founding leaves, and it is
        // NOT the one a market-address namespace would name. Pinned here so a
        // later edit that quietly re-conflates the two fails loudly instead of
        // making the campaign vacuous again.
        assert_ne!(
            hoard,
            Pubkey::find_program_address(
                &CustodyVaultSeedsV1::new(
                    fixture.market.to_bytes(),
                    fixture.release_set,
                    fixture.market.to_bytes(),
                    CompartmentV1::HoardPrincipal,
                )
                .as_slices(),
                &CUSTODY_PROGRAM_ID,
            )
            .0,
            "a Hoard namespaced by the Market address is a different account"
        );
        // NOTHING plants the replay. Until this campaign it added a Claims-role
        // `CustodyReplayV1` straight into the ledger -- a prestate that, before
        // the role entered the replay seeds, NO route in the tree could produce:
        // the founding realizes a Trading-role replay at the namespace and
        // legacy Open a Core-role one, and a replay's role is immutable once
        // written. The redemption was therefore green against an account that
        // could not exist. `create_claims_custody_replay` now creates it by
        // executing the real route against the real ELFs, and every terminal
        // test starts by doing so.
        //
        // The shape the fixture used to assert is still asserted, one layer up:
        // the created replay must admit this exact transfer.
        replay_admissible_from_creation(custody_request, fixture.actor.pubkey());
        add_account(
            &mut test,
            collateral_mint,
            TOKEN_PROGRAM_ID,
            mint_data(
                COption::None,
                INITIAL_RECIPIENT_ATOMS + initial_hoard_atoms,
                6,
            ),
        );
        add_account(
            &mut test,
            hoard,
            TOKEN_PROGRAM_ID,
            token_account_data(collateral_mint, custody_authority, initial_hoard_atoms),
        );
        add_account(
            &mut test,
            recipient,
            TOKEN_PROGRAM_ID,
            token_account_data(
                collateral_mint,
                fixture.actor.pubkey(),
                INITIAL_RECIPIENT_ATOMS,
            ),
        );
        add_account(&mut test, custody_caller, system_program::ID, Vec::new());
        fixture.terminal_accounts = Some(TerminalFixture {
            certificate: terminal_certificate.expect("terminal certificate"),
            realm_raw,
            realm_staging,
            custody_caller,
            custody_replay,
            collateral_mint,
            hoard,
            recipient,
            custody_authority,
        });
        assert_eq!(
            request_bytes(&fixture, RepresentationActionV2::RedeemTerminal, 0),
            terminal_request
        );
        let outer = outer_caller_authority(&terminal_request, fixture.market, fixture.release_set);
        add_account(&mut test, outer, system_program::ID, Vec::new());
        let losing_request =
            request_bytes_for_selected(&fixture, RepresentationActionV2::RedeemTerminal, 0, 0);
        let losing_outer =
            outer_caller_authority(&losing_request, fixture.market, fixture.release_set);
        add_account(&mut test, losing_outer, system_program::ID, Vec::new());
    } else {
        for (action, revision) in [
            (RepresentationActionV2::IssueStructured, 0),
            (RepresentationActionV2::UnwrapStructured, 1),
            (RepresentationActionV2::Denominate, 2),
            (RepresentationActionV2::Reconstitute, 3),
        ] {
            let bytes = request_bytes(&fixture, action, revision);
            let outer = outer_caller_authority(&bytes, fixture.market, fixture.release_set);
            add_account(&mut test, outer, system_program::ID, Vec::new());
        }
    }
    (test, fixture)
}

fn claims_accounts_for_selected(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
    selected_outcome: u32,
    descriptor_records: Option<(Pubkey, Pubkey)>,
    graph_records: Option<(Pubkey, Pubkey)>,
) -> Vec<AccountMeta> {
    let request =
        request_bytes_for_selected(fixture, action, representation_revision, selected_outcome);
    let decoded_request =
        RepresentationRequestV2::decode(&request).expect("canonical fixture request");
    let caller = outer_caller_authority(&request, fixture.market, fixture.release_set);
    let structured = matches!(
        action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    let terminal = action == RepresentationActionV2::RedeemTerminal;
    let claims_active = action.selected_outcome();
    let actor_position_active = matches!(
        action,
        RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
    );
    let (descriptor_raw, descriptor_staging) =
        descriptor_records.unwrap_or((fixture.descriptor_raw, fixture.descriptor_staging));
    let (graph_raw, graph_staging) =
        graph_records.unwrap_or((fixture.graph_raw, fixture.graph_staging));
    let mut metas = vec![
        AccountMeta::new_readonly(caller, false),
        AccountMeta::new_readonly(TEST_CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.caller_programdata, false),
        AccountMeta::new_readonly(fixture.actor.pubkey(), true),
        AccountMeta::new_readonly(fixture.representation_authority, false),
        AccountMeta::new_readonly(descriptor_raw, false),
        AccountMeta::new_readonly(descriptor_staging, false),
        AccountMeta::new_readonly(graph_raw, false),
        AccountMeta::new_readonly(graph_staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new(fixture.representation_replay, false),
        if claims_active {
            AccountMeta::new(fixture.aggregate, false)
        } else {
            AccountMeta::new_readonly(fixture.aggregate, false)
        },
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.claims_programdata, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.market, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        if structured {
            AccountMeta::new(fixture.receipt_mint, false)
        } else {
            AccountMeta::new_readonly(fixture.receipt_mint, false)
        },
        if structured {
            AccountMeta::new(fixture.actor_receipt, false)
        } else {
            AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)
        },
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        if actor_position_active {
            AccountMeta::new(fixture.actor_position, false)
        } else {
            AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)
        },
        AccountMeta::new_readonly(fixture.linked_basis_record, false),
        AccountMeta::new_readonly(fixture.linked_basis_staging, false),
        AccountMeta::new_readonly(fixture.product_record, false),
        AccountMeta::new_readonly(fixture.product_staging, false),
        AccountMeta::new_readonly(fixture.result_domain_record, false),
        AccountMeta::new_readonly(fixture.result_domain_staging, false),
        AccountMeta::new_readonly(fixture.portfolio_record, false),
        AccountMeta::new_readonly(fixture.portfolio_staging, false),
    ];
    assert_eq!(metas.len(), RATIONAL_BASE_ACCOUNT_COUNT_V2);
    let selected_outcome = decoded_request.header().selected_outcome;
    let physical_assets = if action.selected_outcome() {
        vec![
            *fixture
                .assets
                .get(usize::try_from(selected_outcome).expect("selected outcome index"))
                .expect("selected fixture asset"),
        ]
    } else {
        fixture.assets.to_vec()
    };
    for asset in physical_assets {
        let selected = action.selected_outcome();
        metas.extend([
            if selected {
                AccountMeta::new(asset.position, false)
            } else {
                AccountMeta::new_readonly(asset.position, false)
            },
            if selected {
                AccountMeta::new(asset.mint, false)
            } else {
                AccountMeta::new_readonly(asset.mint, false)
            },
            AccountMeta::new(asset.actor_token, false),
            if structured {
                AccountMeta::new(asset.structured_token, false)
            } else {
                AccountMeta::new_readonly(asset.structured_token, false)
            },
        ]);
    }
    assert_eq!(
        metas.len(),
        RATIONAL_BASE_ACCOUNT_COUNT_V2
            + usize::try_from(decoded_request.header().asset_count).expect("physical asset width")
                * RATIONAL_ASSET_ACCOUNT_COUNT_V2
    );
    if terminal {
        let terminal = fixture.terminal_accounts.expect("terminal fixture");
        metas.extend([
            AccountMeta::new_readonly(terminal.custody_caller, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.custody_programdata, false),
            AccountMeta::new_readonly(terminal.certificate, false),
            AccountMeta::new_readonly(RESOLUTION_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.resolution_programdata, false),
            AccountMeta::new_readonly(terminal.realm_raw, false),
            AccountMeta::new_readonly(terminal.realm_staging, false),
            AccountMeta::new(terminal.custody_replay, false),
            AccountMeta::new_readonly(terminal.collateral_mint, false),
            AccountMeta::new(terminal.hoard, false),
            AccountMeta::new(terminal.recipient, false),
            AccountMeta::new_readonly(terminal.custody_authority, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ]);
        assert_eq!(
            metas.len(),
            RATIONAL_BASE_ACCOUNT_COUNT_V2
                + usize::try_from(decoded_request.header().asset_count)
                    .expect("physical asset width")
                    * RATIONAL_ASSET_ACCOUNT_COUNT_V2
                + RATIONAL_TERMINAL_ACCOUNT_COUNT_V2
        );
    }
    metas
}

fn wrapper_instruction(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
    fail_after: bool,
    descriptor_records: Option<(Pubkey, Pubkey)>,
    graph_records: Option<(Pubkey, Pubkey)>,
) -> Instruction {
    wrapper_instruction_for_selected(
        fixture,
        action,
        representation_revision,
        WINNER,
        fail_after,
        descriptor_records,
        graph_records,
    )
}

#[allow(clippy::too_many_arguments)]
fn wrapper_instruction_for_selected(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
    selected_outcome: u32,
    fail_after: bool,
    descriptor_records: Option<(Pubkey, Pubkey)>,
    graph_records: Option<(Pubkey, Pubkey)>,
) -> Instruction {
    let request =
        request_bytes_for_selected(fixture, action, representation_revision, selected_outcome);
    let mut accounts = vec![AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)];
    accounts.extend(claims_accounts_for_selected(
        fixture,
        action,
        representation_revision,
        selected_outcome,
        descriptor_records,
        graph_records,
    ));
    let mut data = Vec::with_capacity(request.len() + 1);
    data.push(u8::from(fail_after));
    data.extend_from_slice(&request);
    Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

fn unique_account_count(instruction: &Instruction) -> usize {
    let mut addresses = vec![instruction.program_id];
    for account in &instruction.accounts {
        if !addresses.contains(&account.pubkey) {
            addresses.push(account.pubkey);
        }
    }
    addresses.len()
}

/// Compute-unit limit this campaign's routes need, asked for on the wire.
///
/// 1.4M is the runtime maximum and the figure the forced budget used to install,
/// so every published CU number stays comparable across the migration.
const CAMPAIGN_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// The ComputeBudget instruction every submitted transaction carries.
///
/// One author, and every submission and every wire MEASUREMENT goes through it:
/// a packet figure that omits what a real transaction carries is a packet figure
/// for a transaction nobody sends.
fn compute_unit_limit_instruction() -> Instruction {
    solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(
        CAMPAIGN_COMPUTE_UNIT_LIMIT,
    )
}

fn legacy_wire_bytes(payer: Pubkey, instruction: Instruction, _hash: Hash) -> usize {
    let message = solana_message::legacy::Message::new(
        &[compute_unit_limit_instruction(), instruction],
        Some(&payer),
    );
    1 + usize::from(message.header.num_required_signatures) * 64 + message.serialize().len()
}

fn no_lookup_v0_wire_bytes(payer: Pubkey, instruction: Instruction, hash: Hash) -> usize {
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &payer,
            &[compute_unit_limit_instruction(), instruction],
            &[],
            hash,
        )
        .expect("uncompressed v0"),
    );
    1 + 2 * 64 + message.serialize().len()
}

fn live_lookup_v0_wire_bytes(
    payer: Pubkey,
    instruction: Instruction,
    hash: Hash,
    table: Pubkey,
    addresses: &[Pubkey],
) -> usize {
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &payer,
            &[compute_unit_limit_instruction(), instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            hash,
        )
        .expect("compressed v0"),
    );
    1 + 2 * 64 + message.serialize().len()
}

fn lookup_addresses(payer: Pubkey, actor: Pubkey, instructions: &[Instruction]) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for instruction in instructions {
        if instruction.program_id != payer
            && instruction.program_id != actor
            && !addresses.contains(&instruction.program_id)
        {
            addresses.push(instruction.program_id);
        }
        for account in &instruction.accounts {
            if account.pubkey != payer
                && account.pubkey != actor
                && !addresses.contains(&account.pubkey)
            {
                addresses.push(account.pubkey);
            }
        }
    }
    addresses
}

async fn process_legacy(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &str,
) -> u64 {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[compute_unit_limit_instruction(), instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed ALT transaction")
        .to_string();
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message_data().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("ALT lifecycle processing");
    let accepted = processed.result.is_ok();
    let failure = processed.result.err().map(|error| format!("{error:?}"));
    let (logs, units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    assert!(accepted, "ALT lifecycle must commit");
    units
}

async fn create_live_lookup_table(
    context: &mut ProgramTestContext,
    addresses: &[Pubkey],
    label_prefix: &str,
) -> (Pubkey, Vec<u64>) {
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("make lookup-table slot recent");
    let payer = context.payer.pubkey();
    let (create, table) = create_lookup_table(payer, payer, clock.slot);
    let mut compute_units = vec![
        process_legacy(
            context,
            create,
            &format!("{label_prefix}: create lookup table"),
        )
        .await,
    ];
    for (index, chunk) in addresses.chunks(20).enumerate() {
        compute_units.push(
            process_legacy(
                context,
                extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
                &format!("{label_prefix}: extend lookup table {index}"),
            )
            .await,
        );
    }
    let extension_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(extension_clock.slot + 1)
        .expect("activate lookup addresses");
    (table, compute_units)
}

async fn submit_v0(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Result<Submission, BanksClientError> {
    submit_v0_instructions(context, fixture, &[instruction], table, addresses, label).await
}

/// The same v0 submission over an explicit instruction list.
///
/// Split out rather than duplicated so the campaign keeps ONE submission path.
/// The only caller that needs more than one instruction is the common-Hot
/// campaign, which must carry a ComputeBudget heap grant: `DCLTHOT3` is on
/// `declares_extended_heap_profile_v1`'s list, and that list makes a grant
/// ADMISSIBLE rather than automatic -- the route asks
/// `require_declared_heap_ceiling_above_default_v1` to refuse `TradingSbfError::HeapFrame`
/// by name when it did not arrive. The remedy is the caller's, and in this
/// family the caller is whoever assembles the transaction: the operator returns
/// a bare `Instruction`, not a transaction, so nothing between it and the wire
/// was adding one.
/// The common-Hot submission, carrying the ComputeBudget heap grant the route
/// declares and refuses `HeapFrame` without.
///
/// Every other campaign in this file submits one bare instruction, because
/// every other route fits the protocol-default 32 KiB. `DCLTHOT3` does not: a
/// caller who invokes Trading directly makes two Registry reauthentication CPIs
/// and holds their frames against an allocator that never frees. The grant is
/// best-effort by construction, so the transaction has to ask.
async fn submit_v0_with_heap(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Result<Submission, BanksClientError> {
    let heap = solana_compute_budget_interface::ComputeBudgetInstruction::request_heap_frame(
        dclutch_capability_program_contract::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1,
    );
    submit_v0_instructions(
        context,
        fixture,
        &[heap, instruction],
        table,
        addresses,
        label,
    )
    .await
}

async fn submit_v0_instructions(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instructions: &[Instruction],
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Result<Submission, BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut carried = std::vec![compute_unit_limit_instruction()];
    carried.extend_from_slice(instructions);
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            &carried,
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 message"),
    );
    let transaction = VersionedTransaction::try_new(message, &[&context.payer, &fixture.actor])
        .expect("signed v0 transaction");
    let signature = transaction
        .signatures
        .first()
        .ok_or(BanksClientError::ClientError("unsigned transaction"))?
        .to_string();
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message.serialize().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await?;
    let accepted = processed.result.is_ok();
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let (compute_units, logs) = processed
        .metadata
        .map(|metadata| (metadata.compute_units_consumed, metadata.log_messages))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(compute_units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    Ok(Submission {
        accepted,
        compute_units,
        wire_bytes,
        logs,
    })
}

/// Create this Market's Claims-role Custody replay by EXECUTING the route.
///
/// This is the piece the campaign was missing. The redemption's replay used to
/// be a planted account; here it is created on chain by
/// `dclutch-claims-sbf`'s replay-creation route, which forwards a Custody
/// `InitializeReplay` under the Claims caller authority. Every terminal test
/// runs it first, so the whole terminal half of this campaign now stands on a
/// prestate the tree can actually produce.
///
/// The instruction is deliberately LEGACY, with no address-lookup table: a
/// redeemer must be able to create the cursor with one ordinary transaction, and
/// this asserts it fits.
async fn create_claims_custody_replay(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> Submission {
    submit_replay_creation(
        context,
        fixture,
        ReplayCreationOverrides::default(),
        "claims rational-representation-v2: create the Claims-role Custody replay",
    )
    .await
}

/// Accounts a hostile replay-creation submission substitutes.
#[derive(Clone, Copy, Default)]
struct ReplayCreationOverrides {
    /// Stand something else in for the Claims aggregate that owns the namespace.
    aggregate: Option<Pubkey>,
    /// Stand something else in for the canonical Claims-role replay address.
    replay: Option<Pubkey>,
}

async fn submit_replay_creation(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    overrides: ReplayCreationOverrides,
    label: &str,
) -> Submission {
    let terminal = fixture
        .terminal_accounts
        .expect("terminal fixture required for a Claims Custody replay");
    let aggregate_account = observed(context, fixture.aggregate).await;
    let aggregate =
        LiabilityBasisMarketViewV2::decode(&aggregate_account.data).expect("aggregate on chain");
    assert_eq!(
        aggregate.custody_context, fixture.custody_context,
        "the aggregate is the sole persisted owner of the namespace"
    );
    let rent = context.banks_client.get_rent().await.expect("Rent sysvar");
    let request = expected_request_v1(
        aggregate,
        CLAIMS_PROGRAM_ID.to_bytes(),
        fixture.actor.pubkey().to_bytes(),
        market_rent_credit().to_bytes(),
        rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
    )
    .expect("the sole Custody request this route sends");
    assert_eq!(request.caller_role, CustodyCallerRoleV1::Claims);
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    assert_eq!(
        replay, terminal.custody_replay,
        "the created replay must be the account the redemption authenticates"
    );
    // The Trading compartment of the SAME namespace is a different account.
    // Asserted here so every terminal test carries the separation, not just the
    // one adversarial case that attacks it.
    assert_ne!(
        replay,
        Pubkey::find_program_address(
            &CustodyReplaySeedsV1::new(
                request.market,
                request.release_set,
                CustodyCallerRoleV1::Trading,
                request.context,
            )
            .as_slices(),
            &CUSTODY_PROGRAM_ID,
        )
        .0,
        "one namespace, one replay compartment per executing role"
    );
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).expect("release set"),
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        hash(&request.to_bytes().expect("Custody request")).to_bytes(),
    )
    .expect("Claims caller seeds");
    let caller = Pubkey::find_program_address(&caller_seeds.as_slices(), &CLAIMS_PROGRAM_ID).0;
    let instruction = Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts: Vec::from([
            AccountMeta::new_readonly(caller, false),
            AccountMeta::new_readonly(fixture.market, false),
            AccountMeta::new_readonly(fixture.activation_cache, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.claims_programdata, false),
            AccountMeta::new_readonly(terminal.realm_raw, false),
            AccountMeta::new_readonly(terminal.realm_staging, false),
            AccountMeta::new(overrides.replay.unwrap_or(terminal.custody_replay), false),
            AccountMeta::new(fixture.actor.pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new(market_rent_credit(), false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(overrides.aggregate.unwrap_or(fixture.aggregate), false),
        ]),
        data: ClaimsCustodyReplayRequestV1::new(fixture.market.to_bytes())
            .expect("replay-creation request")
            .to_bytes()
            .to_vec(),
    };
    assert_eq!(
        instruction.accounts.len(),
        CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1
    );
    submit_legacy_signed(context, fixture, instruction, label).await
}

/// Submit one legacy transaction signed by the fee payer and the fixture actor.
async fn submit_legacy_signed(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
    label: &str,
) -> Submission {
    submit_legacy_with(context, instruction, &[&fixture.actor], label).await
}

/// The same legacy submission with an explicit extra-signer list.
///
/// Split out rather than duplicated because a hostile that builds its own
/// transaction is a hostile that can pass for the wrong reason. The payer is
/// always a signer; everything else is the caller's, which is what lets one
/// campaign submit both an actor-signed act and a stranger-signed one against
/// the same route without two submission paths.
async fn submit_legacy_with(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    extra_signers: &[&Keypair],
    label: &str,
) -> Submission {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let mut signers: Vec<&Keypair> = std::vec![&context.payer];
    signers.extend_from_slice(extra_signers);
    let transaction = Transaction::new_signed_with_payer(
        &[compute_unit_limit_instruction(), instruction],
        Some(&context.payer.pubkey()),
        &signers,
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed transaction")
        .to_string();
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message_data().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("legacy transaction processing");
    let accepted = processed.result.is_ok();
    let failure = processed.result.err().map(|error| format!("{error:?}"));
    let (logs, compute_units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(compute_units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    Submission {
        accepted,
        compute_units,
        wire_bytes,
        logs,
    }
}

/// The exact custom refusal a submission carries, parsed rather than matched.
///
/// `logs.iter().any(|line| line.contains("Custom(3)"))` is not a refusal
/// assertion -- it accepts `Custom(30)` too (AGENTS.md, refusal codes). The
/// runtime writes the code as the final token of its own line, so parsing that
/// token is exact where a substring is not, and a caller then compares against
/// a discriminant taken off the enum.
fn custom_code(submission: &Submission) -> Option<u32> {
    const MARKER: &str = "custom program error: 0x";
    submission.logs.iter().find_map(|line| {
        let hex = line.split(MARKER).nth(1)?.trim();
        u32::from_str_radix(hex, 16).ok()
    })
}

async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .expect("existing account")
}

/// Observe one coordinate-indexed account across every asset.
///
/// Written as a walk rather than an array literal so the width of the campaign
/// basis is a constant, not a copy-paste count.
async fn observed_per_asset(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    select: impl Fn(&AssetFixture) -> Pubkey,
) -> [Account; K] {
    let mut observations = Vec::with_capacity(K);
    for asset in &fixture.assets {
        observations.push(observed(context, select(asset)).await);
    }
    observations
        .try_into()
        .expect("one observation per representation coordinate")
}

async fn snapshot(context: &mut ProgramTestContext, fixture: &Fixture) -> Snapshot {
    let positions = observed_per_asset(context, fixture, |asset| asset.position).await;
    let shard_mints = observed_per_asset(context, fixture, |asset| asset.mint).await;
    let actor_shards = observed_per_asset(context, fixture, |asset| asset.actor_token).await;
    let structured_shards =
        observed_per_asset(context, fixture, |asset| asset.structured_token).await;
    let obsolete_structured_shards =
        observed_per_asset(context, fixture, |asset| asset.obsolete_structured_ata).await;
    Snapshot {
        replay: observed(context, fixture.representation_replay).await,
        aggregate: observed(context, fixture.aggregate).await,
        actor_position: observed(context, fixture.actor_position).await,
        positions,
        receipt_mint: observed(context, fixture.receipt_mint).await,
        actor_receipt: observed(context, fixture.actor_receipt).await,
        shard_mints,
        actor_shards,
        structured_shards,
        obsolete_structured_shards,
        custody_replay: match fixture.terminal_accounts {
            Some(value) => Some(observed(context, value.custody_replay).await),
            None => None,
        },
        hoard: match fixture.terminal_accounts {
            Some(value) => Some(observed(context, value.hoard).await),
            None => None,
        },
        recipient: match fixture.terminal_accounts {
            Some(value) => Some(observed(context, value.recipient).await),
            None => None,
        },
    }
}

fn token_amount(account: &Account) -> u64 {
    TokenAccount::parse(&account.data)
        .expect("Token Account")
        .amount
}

fn mint_supply(account: &Account) -> u64 {
    SplMint::unpack_from_slice(account.data.get(..SplMint::LEN).expect("base Mint bytes"))
        .expect("Mint")
        .supply
}

fn mint_decimals(account: &Account) -> u8 {
    SplMint::unpack_from_slice(account.data.get(..SplMint::LEN).expect("base Mint bytes"))
        .expect("Mint")
        .decimals
}

fn assert_account_content_eq(actual: &Account, expected: &Account) {
    assert_eq!(actual.lamports, expected.lamports);
    assert_eq!(actual.owner, expected.owner);
    assert_eq!(actual.executable, expected.executable);
    assert_eq!(actual.data, expected.data);
}

fn replay_revision(account: &Account) -> u64 {
    assert_eq!(account.owner, CLAIMS_PROGRAM_ID);
    assert_eq!(account.data.len(), RATIONAL_REPLAY_BYTES_V2);
    assert_eq!(
        account.data.get(..8),
        Some(RATIONAL_REPLAY_MAGIC_V2.as_slice())
    );
    u64::from_le_bytes(
        account
            .data
            .get(80..88)
            .expect("replay revision")
            .try_into()
            .expect("revision width"),
    )
}

fn lbv2_revision(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(
        bytes
            .get(16..24)
            .expect("LBV2 revision")
            .try_into()
            .expect("LBV2 revision width"),
    )
}

fn lbv2_position_quantity(bytes: &[u8], outcome: u32) -> u64 {
    let index = usize::try_from(outcome).expect("outcome index");
    let offset = 128_usize
        .checked_add(index.checked_mul(8).expect("quantity offset"))
        .expect("quantity offset");
    u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .expect("LBV2 Position quantity")
            .try_into()
            .expect("LBV2 quantity width"),
    )
}

fn packet_measurements(
    payer: Pubkey,
    instruction: &Instruction,
    blockhash: Hash,
    table: Pubkey,
    addresses: &[Pubkey],
) -> (usize, usize, usize) {
    let legacy = legacy_wire_bytes(payer, instruction.clone(), blockhash);
    let no_alt = no_lookup_v0_wire_bytes(payer, instruction.clone(), blockhash);
    let live_alt =
        live_lookup_v0_wire_bytes(payer, instruction.clone(), blockhash, table, addresses);
    assert!(legacy > PACKET_LIMIT, "legacy must honestly overflow");
    assert!(
        no_alt > PACKET_LIMIT,
        "v0 without ALT must honestly overflow"
    );
    // The live-ALT form is NOT asserted here. At the campaign basis one of the
    // two route shapes fits and the other does not, and which is which is a
    // measurement this campaign exists to take -- see
    // `the_full_width_structured_frame_does_not_fit_a_packet_at_k_three`.
    (legacy, no_alt, live_alt)
}

#[tokio::test]
async fn real_sbf_open_actions_are_exact_and_conserved() {
    let (test, fixture) = fixture(false);
    let mut context = test.start_with_context().await;
    let issue = wrapper_instruction(
        &fixture,
        RepresentationActionV2::IssueStructured,
        0,
        false,
        None,
        None,
    );
    let unwrap = wrapper_instruction(
        &fixture,
        RepresentationActionV2::UnwrapStructured,
        1,
        false,
        None,
        None,
    );
    let denominate = wrapper_instruction(
        &fixture,
        RepresentationActionV2::Denominate,
        2,
        false,
        None,
        None,
    );
    let reconstitute = wrapper_instruction(
        &fixture,
        RepresentationActionV2::Reconstitute,
        3,
        false,
        None,
        None,
    );
    let mut obsolete_ata_substitution = issue.clone();
    let first_asset = fixture.assets.first().expect("first asset");
    let obsolete_meta = obsolete_ata_substitution
        .accounts
        .iter_mut()
        .find(|meta| meta.pubkey == first_asset.structured_token)
        .expect("canonical structured custody meta");
    obsolete_meta.pubkey = first_asset.obsolete_structured_ata;
    assert_eq!(
        issue.accounts.len(),
        1 + RATIONAL_BASE_ACCOUNT_COUNT_V2
            + usize::try_from(OUTCOME_COUNT).expect("outcome width")
                * RATIONAL_ASSET_ACCOUNT_COUNT_V2
    );
    assert_eq!(
        issue.data.len(),
        1 + REQUEST_STRUCTURED_HEADER_BYTES_V3 + K * ASSET_BYTES_V3
    );
    let payer = context.payer.pubkey();
    let addresses = lookup_addresses(
        payer,
        fixture.actor.pubkey(),
        &[
            issue.clone(),
            unwrap.clone(),
            denominate.clone(),
            reconstitute.clone(),
            obsolete_ata_substitution.clone(),
        ],
    );
    let (table, lookup_cu) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: open actions",
    )
    .await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("post-ALT blockhash");
    let (legacy, no_alt, live_alt) =
        packet_measurements(payer, &issue, blockhash, table, &addresses);
    eprintln!(
        "Rational V2 structured packet preflight: request={}, claims-frame={}, outer-metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, ALT-CU={lookup_cu:?}",
        REQUEST_STRUCTURED_HEADER_BYTES_V3 + K * ASSET_BYTES_V3,
        RATIONAL_BASE_ACCOUNT_COUNT_V2
            + usize::try_from(OUTCOME_COUNT).expect("outcome width")
                * RATIONAL_ASSET_ACCOUNT_COUNT_V2,
        issue.accounts.len(),
        unique_account_count(&issue),
        legacy,
        no_alt,
        live_alt,
    );
    let before = snapshot(&mut context, &fixture).await;
    assert_eq!(mint_decimals(&before.receipt_mint), RECEIPT_DECIMALS);
    for (actual, expected) in before.shard_mints.iter().zip(SHARD_DECIMALS) {
        assert_eq!(mint_decimals(actual), expected);
    }

    let obsolete = submit_v0(
        &mut context,
        &fixture,
        obsolete_ata_substitution,
        table,
        &addresses,
        "claims rational-representation-v2: issue against an obsolete structured ATA",
    )
    .await
    .expect("obsolete ATA substitution transaction");
    assert!(!obsolete.accepted, "obsolete Structured ATA must refuse");
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before,
        "obsolete ATA substitution must roll back every resource"
    );

    let issued = submit_v0(
        &mut context,
        &fixture,
        issue.clone(),
        table,
        &addresses,
        "claims rational-representation-v2: IssueStructured commits",
    )
    .await
    .expect("IssueStructured transaction");
    if !issued.accepted {
        eprintln!("IssueStructured refusal logs:\n{}", issued.logs.join("\n"));
    }
    assert!(issued.accepted, "IssueStructured must commit");
    // THE WALL IS GONE, and this assertion is the one that had to invert to say
    // so. It read `> PACKET_LIMIT` and was true every day of its life: the K = 3
    // full-width Claims-direct frame measured 1,397 bytes against a 1,232-byte
    // cluster packet limit with the ALT already applied, so a K = 3 Product
    // could be denominated and redeemed on a cluster and never issued or
    // unwrapped there. Physical ABI v3 cut the request from 968 bytes to 576 --
    // an action-conditional header AND three re-derived per-coordinate keys off
    // the wire -- and the frame is now UNDER the limit. Recorded as an equality
    // against the limit rather than a bare `<=`, for the same reason its
    // predecessor was an inequality it stated out loud: a bound that only says
    // the route works cannot report the day it stops.
    assert!(
        issued.wire_bytes <= PACKET_LIMIT,
        "the K = 3 full-width frame is measured at {} bytes, over the {PACKET_LIMIT}-byte limit \
         it was under; the packet wall this campaign recorded as gone has come back",
        issued.wire_bytes,
    );
    assert!(
        issued
            .logs
            .iter()
            .any(|log| log == &format!("Program {TOKEN_PROGRAM_ID} success")),
        "real Token-2022 must execute"
    );
    let after_issue = snapshot(&mut context, &fixture).await;
    assert_eq!(after_issue.aggregate, before.aggregate);
    assert_eq!(after_issue.actor_position, before.actor_position);
    assert_eq!(after_issue.positions, before.positions);
    assert_eq!(replay_revision(&after_issue.replay), 1);
    assert_eq!(mint_supply(&after_issue.receipt_mint), RECEIPT_SUPPLY + 1);
    assert_eq!(token_amount(&after_issue.actor_receipt), 1);
    for (index, (actor, structured)) in actor_shards_after_issue()
        .into_iter()
        .zip(structured_shards_after_issue())
        .enumerate()
    {
        let supply = mint_supply(after_issue.shard_mints.get(index).expect("shard Mint"));
        assert_eq!(supply, shard_supply(index));
        assert_eq!(
            token_amount(after_issue.actor_shards.get(index).expect("actor shards")),
            actor
        );
        assert_eq!(
            token_amount(
                after_issue
                    .structured_shards
                    .get(index)
                    .expect("structured shards"),
            ),
            structured
        );
        assert_eq!(actor + structured, supply, "no hidden shard remainder");
    }

    let unwrapped = submit_v0(
        &mut context,
        &fixture,
        unwrap,
        table,
        &addresses,
        "claims rational-representation-v2: UnwrapStructured commits",
    )
    .await
    .expect("UnwrapStructured transaction");
    if !unwrapped.accepted {
        eprintln!(
            "UnwrapStructured refusal logs:\n{}",
            unwrapped.logs.join("\n")
        );
    }
    assert!(unwrapped.accepted, "UnwrapStructured must commit");
    assert_eq!(
        unwrapped.wire_bytes, issued.wire_bytes,
        "Issue and Unwrap ride the identical frame"
    );
    let after_unwrap = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after_unwrap.replay), 2);
    assert_eq!(after_unwrap.aggregate, before.aggregate);
    assert_eq!(after_unwrap.actor_position, before.actor_position);
    assert_eq!(after_unwrap.positions, before.positions);
    assert_account_content_eq(&after_unwrap.receipt_mint, &before.receipt_mint);
    assert_account_content_eq(&after_unwrap.actor_receipt, &before.actor_receipt);
    for (actual, expected) in after_unwrap.shard_mints.iter().zip(&before.shard_mints) {
        assert_account_content_eq(actual, expected);
    }
    for (actual, expected) in after_unwrap.actor_shards.iter().zip(&before.actor_shards) {
        assert_account_content_eq(actual, expected);
    }
    for (actual, expected) in after_unwrap
        .structured_shards
        .iter()
        .zip(&before.structured_shards)
    {
        assert_account_content_eq(actual, expected);
    }

    let denominated = submit_v0(
        &mut context,
        &fixture,
        denominate,
        table,
        &addresses,
        "claims rational-representation-v2: Denominate commits",
    )
    .await
    .expect("Denominate transaction");
    if !denominated.accepted {
        eprintln!("Denominate refusal logs:\n{}", denominated.logs.join("\n"));
    }
    assert!(denominated.accepted, "Denominate must commit");
    let after_denominate = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after_denominate.replay), 3);
    assert_eq!(lbv2_revision(&after_denominate.aggregate.data), 1);
    assert_eq!(lbv2_revision(&after_denominate.actor_position.data), 1);
    assert_eq!(
        lbv2_position_quantity(&after_denominate.actor_position.data, WINNER),
        1
    );
    assert_eq!(lbv2_revision(&after_denominate.positions[WINNERS].data), 1);
    // Denominate moves one whole claim from the actor's Position to the winning
    // coordinate's custody Position, and mints exactly `DENOMINATOR` shard atoms
    // for it. All three numbers are that one claim, stated in three units.
    assert_eq!(
        lbv2_position_quantity(&after_denominate.positions[WINNERS].data, WINNER),
        CUSTODY_CLAIMS[WINNERS] + 1
    );
    assert_eq!(
        mint_supply(&after_denominate.shard_mints[WINNERS]),
        shard_supply(WINNERS) + DENOMINATOR
    );
    assert_eq!(
        token_amount(&after_denominate.actor_shards[WINNERS]),
        actor_shards()[WINNERS] + DENOMINATOR
    );

    let reconstituted = submit_v0(
        &mut context,
        &fixture,
        reconstitute,
        table,
        &addresses,
        "claims rational-representation-v2: Reconstitute commits",
    )
    .await
    .expect("Reconstitute transaction");
    if !reconstituted.accepted {
        eprintln!(
            "Reconstitute refusal logs:\n{}",
            reconstituted.logs.join("\n")
        );
    }
    assert!(reconstituted.accepted, "Reconstitute must commit");
    let after_reconstitute = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after_reconstitute.replay), 4);
    assert_eq!(lbv2_revision(&after_reconstitute.aggregate.data), 2);
    assert_eq!(lbv2_revision(&after_reconstitute.actor_position.data), 2);
    assert_eq!(
        lbv2_position_quantity(&after_reconstitute.actor_position.data, WINNER),
        2
    );
    assert_eq!(
        lbv2_revision(&after_reconstitute.positions[WINNERS].data),
        2
    );
    assert_eq!(
        lbv2_position_quantity(&after_reconstitute.positions[WINNERS].data, WINNER),
        CUSTODY_CLAIMS[WINNERS]
    );
    for (actual, expected) in after_reconstitute
        .shard_mints
        .iter()
        .zip(&before.shard_mints)
    {
        assert_account_content_eq(actual, expected);
    }
    for (actual, expected) in after_reconstitute
        .actor_shards
        .iter()
        .zip(&before.actor_shards)
    {
        assert_account_content_eq(actual, expected);
    }
    eprintln!(
        "Rational V2 open: request={}, claims-frame={}, outer-metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, issue-v0={}, issue-CU={}, unwrap-v0={}, unwrap-CU={}, denominate-v0={}, denominate-CU={}, reconstitute-v0={}, reconstitute-CU={}, ALT-CU={lookup_cu:?}",
        REQUEST_STRUCTURED_HEADER_BYTES_V3 + K * ASSET_BYTES_V3,
        RATIONAL_BASE_ACCOUNT_COUNT_V2
            + usize::try_from(OUTCOME_COUNT).expect("outcome width")
                * RATIONAL_ASSET_ACCOUNT_COUNT_V2,
        issue.accounts.len(),
        unique_account_count(&issue),
        legacy,
        no_alt,
        live_alt,
        issued.wire_bytes,
        issued.compute_units,
        unwrapped.wire_bytes,
        unwrapped.compute_units,
        denominated.wire_bytes,
        denominated.compute_units,
        reconstituted.wire_bytes,
        reconstituted.compute_units,
    );
}

/// **The one account in this family that had no route home now has one, and the
/// rent goes back to the party who advanced it.**
///
/// `authenticate_or_allocate_replay` creates one cursor per
/// `(descriptor, actor)`, and before `process_replay_close` the seed constant
/// `RATIONAL_REPLAY_SEED_V2` had exactly ONE on-chain use in the whole tree:
/// that derivation. `rational_lifecycle_v2` closes the receipt Mint, every
/// shard Mint, every structured custody account, every Position and every
/// admission, and never reaches this one. One per actor, so the stranded total
/// grows with adoption.
///
/// The arithmetic asserted here is the whole clause: the actor's balance rises
/// by EXACTLY the cursor's lamports, the cursor ends at zero lamports, zero
/// length and System ownership, and no third party is credited a thing. The
/// close is actor-signed by design and the alternative is refused out loud in
/// the request's own doc: a permissionless sweep of a sleeping actor's cursor
/// would be precisely the absent-holder charge this contract forbids.
///
/// The cursor is created by a real `IssueStructured` first, because a cursor
/// planted by a fixture is a shape nothing in the tree has ever produced.
#[tokio::test]
async fn a_spent_replay_cursor_closes_to_its_actor_and_strands_no_rent() {
    use dclutch_claims_sbf::rational_representation_v2::RationalReplayCloseSbfErrorV1;
    use dclutch_rational_representation_v2_contract::{
        RATIONAL_REPLAY_CLOSE_ACCOUNT_COUNT_V1, RationalReplayCloseRequestV1,
    };

    let (test, fixture) = fixture(false);
    let mut context = test.start_with_context().await;
    let issue = wrapper_instruction(
        &fixture,
        RepresentationActionV2::IssueStructured,
        0,
        false,
        None,
        None,
    );
    let payer = context.payer.pubkey();
    let addresses = lookup_addresses(payer, fixture.actor.pubkey(), std::slice::from_ref(&issue));
    let (table, _) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: replay close",
    )
    .await;
    let issued = submit_v0(
        &mut context,
        &fixture,
        issue,
        table,
        &addresses,
        "claims rational-representation-v2: IssueStructured before a replay close",
    )
    .await
    .expect("IssueStructured transaction");
    assert!(
        issued.accepted,
        "the cursor is only a real account after an action creates it: {:?}",
        issued.logs.last()
    );

    let cursor_before = observed(&mut context, fixture.representation_replay).await;
    assert_eq!(cursor_before.owner, CLAIMS_PROGRAM_ID, "cursor is live");
    assert_eq!(cursor_before.data.len(), RATIONAL_REPLAY_BYTES_V2);
    let stranded = cursor_before.lamports;
    assert!(stranded > 0, "an account with no rent strands nothing");
    let actor_before = observed(&mut context, fixture.actor.pubkey())
        .await
        .lamports;

    let close = |descriptor: [u8; 32], named_actor: Pubkey, actor_account: Pubkey| {
        let request = RationalReplayCloseRequestV1::new(descriptor, named_actor.to_bytes())
            .expect("canonical replay close request");
        let accounts = std::vec![
            AccountMeta::new(actor_account, true),
            AccountMeta::new(fixture.representation_replay, false),
        ];
        assert_eq!(accounts.len(), RATIONAL_REPLAY_CLOSE_ACCOUNT_COUNT_V1);
        Instruction {
            program_id: CLAIMS_PROGRAM_ID,
            accounts,
            data: request.to_bytes().to_vec(),
        }
    };

    // HOSTILE 1, `Authority`: the signing account and the actor the request
    // names disagree. This is the conjunct that stops a signature being spent
    // on a close somebody else authored.
    let mut foreign_actor = fixture.actor.pubkey().to_bytes();
    foreign_actor[0] ^= 1;
    let mismatched = submit_legacy_signed(
        &mut context,
        &fixture,
        close(
            fixture.descriptor_id,
            Pubkey::new_from_array(foreign_actor),
            fixture.actor.pubkey(),
        ),
        "claims rational-representation-v2: replay close naming another actor",
    )
    .await;
    assert!(
        !mismatched.accepted,
        "a close whose request names another actor must be refused"
    );
    assert_eq!(
        custom_code(&mismatched),
        Some(RationalReplayCloseSbfErrorV1::Authority as u32),
        "refused as Authority, by name"
    );

    // HOSTILE 2, `Identity`: a stranger names THEMSELVES -- so the account and
    // the request agree and `Authority` passes -- and points at the actor's
    // cursor. The derivation is the only thing left standing between them and
    // somebody else's rent, and it is what refuses. This is the attack the
    // route exists to survive, so it is submitted under the stranger's own
    // signature rather than the actor's.
    let stranger = context.payer.pubkey();
    let stolen = submit_legacy_with(
        &mut context,
        close(fixture.descriptor_id, stranger, stranger),
        &[],
        "claims rational-representation-v2: a stranger reaches for the actor's rent",
    )
    .await;
    assert!(
        !stolen.accepted,
        "a stranger must not close another actor's cursor"
    );
    assert_eq!(
        custom_code(&stolen),
        Some(RationalReplayCloseSbfErrorV1::Identity as u32),
        "refused as Identity: the cursor a stranger derives is not the one they were handed"
    );

    // HOSTILE 3, `Identity`: the right actor, the wrong descriptor. The other
    // half of the same derivation.
    let mut other_descriptor = fixture.descriptor_id;
    other_descriptor[0] ^= 1;
    let wrong_coordinate = submit_legacy_signed(
        &mut context,
        &fixture,
        close(
            other_descriptor,
            fixture.actor.pubkey(),
            fixture.actor.pubkey(),
        ),
        "claims rational-representation-v2: replay close at another descriptor",
    )
    .await;
    assert!(
        !wrong_coordinate.accepted,
        "a cursor that does not derive must be refused"
    );
    assert_eq!(
        custom_code(&wrong_coordinate),
        Some(RationalReplayCloseSbfErrorV1::Identity as u32),
        "refused as Identity, by name"
    );
    assert_eq!(
        observed(&mut context, fixture.representation_replay)
            .await
            .lamports,
        stranded,
        "no refusal may move a lamport"
    );

    let closed = submit_legacy_signed(
        &mut context,
        &fixture,
        close(
            fixture.descriptor_id,
            fixture.actor.pubkey(),
            fixture.actor.pubkey(),
        ),
        "claims rational-representation-v2: the actor reclaims its replay rent",
    )
    .await;
    assert!(
        closed.accepted,
        "the actor must be able to reclaim its own rent: {:?}",
        closed.logs.last()
    );

    let cursor_after = context
        .banks_client
        .get_account(fixture.representation_replay)
        .await
        .expect("account query");
    match cursor_after {
        None => {}
        Some(account) => {
            assert_eq!(account.lamports, 0, "a closed cursor keeps nothing");
            assert!(account.data.is_empty(), "a closed cursor keeps no bytes");
            assert_eq!(account.owner, solana_sdk_ids::system_program::ID);
        }
    }
    let actor_after = observed(&mut context, fixture.actor.pubkey())
        .await
        .lamports;
    assert_eq!(
        actor_after - actor_before,
        stranded,
        "the actor is credited EXACTLY the cursor's rent -- no split, no reward, no residue"
    );
    eprintln!(
        "Rational V2 replay close: reclaimed={stranded} lamports to the actor, CU={:?}",
        closed.compute_units
    );
}

#[tokio::test]
async fn current_common_hot_executes_issue_and_selected_denominate_through_real_elves() {
    let trading = trading_artifact();
    let (test, fixture) = fixture_with_basis_and_trading(
        TerminalV1::None,
        ReceiptMintRoles::Both,
        ProductBasisProfileV1::Categorical,
        Some(trading),
        CollateralAdapterSelectionV1::Cohort13ZeroExtension,
    );
    let mut context = test.start_with_context().await;

    let issue_plan = common_hot_open::plan_issue(&mut context, &fixture).await;
    let issue_bundle = common_hot_open::build_hot(
        &mut context,
        &fixture,
        RepresentationActionV2::IssueStructured,
        &issue_plan.family_request,
        &issue_plan.claims_child,
    )
    .await;
    common_hot_open::assert_public_issue_outer(&mut context, &fixture, &issue_plan, &issue_bundle)
        .await;
    common_hot_open::install(&mut context, &issue_bundle);
    let before = snapshot(&mut context, &fixture).await;
    let issue = issue_bundle.hot_instruction.clone();
    let issue_addresses = lookup_addresses(
        context.payer.pubkey(),
        fixture.actor.pubkey(),
        std::slice::from_ref(&issue),
    );
    let (issue_table, issue_alt_cu) = create_live_lookup_table(
        &mut context,
        &issue_addresses,
        "Trading common-Hot Rational IssueStructured",
    )
    .await;

    // THE PROPERTY THIS HOSTILE RIDES, asserted where it cannot be a universal
    // donor. The child caller authority is a PDA over the exact projected child
    // request, and that request carries the family request's SHA-256 as its
    // parent context. So one flipped family byte moves the child request, its
    // digest, and the derived address -- and the on-chain half below is the
    // consequence rather than the claim. Stated host-side because a chain
    // assertion made during a wall era proves only that something refused.
    assert_eq!(
        RepresentationRequestV2::decode(&issue_plan.claims_child.instruction.data)
            .expect("canonical child request")
            .header()
            .parent_context,
        hash(&issue_plan.family_request).to_bytes(),
        "the child request's parent context is the family request's digest",
    );
    let mut substituted_digest = issue.clone();
    *substituted_digest
        .data
        .last_mut()
        .expect("nonempty family request") ^= 1;
    let refused = submit_v0_with_heap(
        &mut context,
        &fixture,
        substituted_digest,
        issue_table,
        &issue_addresses,
        "Trading common-Hot Rational IssueStructured substituted request digest refuses",
    )
    .await
    .expect("hostile common-Hot transaction");
    assert!(!refused.accepted, "substituted family request must refuse");
    // PARSED, not matched. `line.contains("Custom(16387)")` also accepts
    // `Custom(163870)` (AGENTS.md, refusal codes) -- and it accepted nothing at
    // all here, because the runtime writes the code as `custom program error:
    // 0x4003` in the program-failure line and renders `Custom(N)` only in the
    // transaction error, which is not in `logs`. So this assertion failed while
    // the route refused with exactly the discriminant it names.
    // AND IT NAMES `Release`, not `Content`. That is a correction, not a
    // relaxation: the substitution is caught by the BINDING, not by a content
    // check. `claims_composition_v3.rs:172-184` derives the child caller
    // authority from the exact projected request and refuses `Release` when the
    // frame's coordinate 0 is not that address, so a flipped family byte cannot
    // present a frame for the request it actually carries. `Content` was this
    // island's expectation for as long as the route refused before ever
    // reaching the binding -- which is every day of its life until 2026-09-01,
    // and is why the expectation was never tested.
    assert_eq!(
        custom_code(&refused),
        Some(dclutch_trading_sbf::TradingSbfError::Release as u32),
        "a substituted family request must fail the caller-authority binding: {}",
        refused.logs.join("\n")
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before,
        "a substituted common-Hot request rolls back Claims and Token state"
    );

    let issued = submit_v0_with_heap(
        &mut context,
        &fixture,
        issue.clone(),
        issue_table,
        &issue_addresses,
        "Trading common-Hot Rational IssueStructured commits",
    )
    .await
    .expect("real Trading IssueStructured transaction");
    if !issued.accepted {
        eprintln!(
            "common-Hot IssueStructured refusal:\n{}",
            issued.logs.join("\n")
        );
    }
    assert!(issued.accepted, "real Trading → Claims IssueStructured");
    assert!(
        issued
            .logs
            .iter()
            .any(|line| line == &format!("Program {CLAIMS_PROGRAM_ID} success")),
        "real Claims ELF must return through Trading"
    );
    assert!(
        issued
            .logs
            .iter()
            .any(|line| line == &format!("Program {TOKEN_PROGRAM_ID} success")),
        "real Token-2022 must commit the receipt and shard transfers"
    );
    let after_issue = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after_issue.replay), 1);
    assert_eq!(after_issue.aggregate, before.aggregate);
    assert_eq!(after_issue.actor_position, before.actor_position);
    assert_eq!(after_issue.positions, before.positions);
    assert_eq!(mint_supply(&after_issue.receipt_mint), RECEIPT_SUPPLY + 1);
    assert_eq!(token_amount(&after_issue.actor_receipt), 1);
    for (index, (actor, structured)) in actor_shards_after_issue()
        .into_iter()
        .zip(structured_shards_after_issue())
        .enumerate()
    {
        let supply = mint_supply(after_issue.shard_mints.get(index).expect("shard Mint"));
        assert_eq!(supply, shard_supply(index));
        assert_eq!(
            token_amount(after_issue.actor_shards.get(index).expect("actor shard")),
            actor
        );
        assert_eq!(
            token_amount(
                after_issue
                    .structured_shards
                    .get(index)
                    .expect("Structured shard"),
            ),
            structured
        );
        assert_eq!(actor + structured, supply, "no hidden shard remainder");
    }

    let denominate_plan = common_hot_open::plan_denominate(&mut context, &fixture).await;
    let denominate_bundle = common_hot_open::build_hot(
        &mut context,
        &fixture,
        RepresentationActionV2::Denominate,
        &denominate_plan.family_request,
        &denominate_plan.claims_child,
    )
    .await;
    common_hot_open::assert_public_denominate_outer(
        &mut context,
        &fixture,
        &denominate_plan,
        &denominate_bundle,
    )
    .await;
    common_hot_open::install(&mut context, &denominate_bundle);
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        after_issue,
        "installing immutable action artifacts does not mutate protocol state"
    );
    let denominate = denominate_bundle.hot_instruction.clone();
    let denominate_addresses = lookup_addresses(
        context.payer.pubkey(),
        fixture.actor.pubkey(),
        std::slice::from_ref(&denominate),
    );
    let (denominate_table, denominate_alt_cu) = create_live_lookup_table(
        &mut context,
        &denominate_addresses,
        "Trading common-Hot Rational Denominate",
    )
    .await;
    let denominated = submit_v0_with_heap(
        &mut context,
        &fixture,
        denominate.clone(),
        denominate_table,
        &denominate_addresses,
        "Trading common-Hot Rational Denominate commits",
    )
    .await
    .expect("real Trading Denominate transaction");
    if !denominated.accepted {
        eprintln!(
            "common-Hot Denominate refusal:\n{}",
            denominated.logs.join("\n")
        );
    }
    assert!(denominated.accepted, "real Trading → Claims Denominate");
    // THE HOT SELECTED FRAME DOES NOT FIT A PACKET EITHER, and this is the
    // first run that could say so.
    //
    // This assertion read `wire_bytes <= PACKET_LIMIT` and called the frame
    // "honestly signable at K=3". It had never been evaluated -- the route
    // refused upstream every day of its life -- and it is false: the Trading
    // common-Hot Denominate is 1,253 bytes against Solana's 1,232-byte
    // PACKET_DATA_BYTES, over by 21, as a v0 message on a live Address Lookup
    // Table. The 1,061 figure recorded for a selected action elsewhere is the
    // CLAIMS-DIRECT frame; the Hot route carries the Trading envelope and its
    // own account frame on top of it, and nothing had measured that.
    //
    // Recorded as an EQUALITY for the reason the Structured island already
    // gives for its own packet witness: a claim that only says "still broken"
    // would not notice the frame getting worse. ProgramTest has no MTU, so the
    // transaction below commits and its conservation is real evidence; what
    // this number says is that the same transaction does not cross a cluster at
    // K = 3.
    // AND NOW IT DOES FIT. This assertion has been rewritten twice, and both
    // rewrites are the same discipline: an EQUALITY, so that the number moving
    // is itself the failure. It first read `wire_bytes <= PACKET_LIMIT` and had
    // never been evaluated, because the route refused upstream every day of its
    // life; the first run that reached it measured 1,253 against 1,232 and the
    // assertion became the exact pair below. Physical ABI v3 then took the
    // Denominate request from 648 bytes to 444 -- an action-conditional header
    // and three re-derived per-coordinate keys off the wire -- and the frame
    // measures 1,049. It is under the limit by 183 bytes, on a v0 message
    // against a live Address Lookup Table, carrying the set_compute_unit_limit a
    // real transaction always pays.
    assert_eq!(
        (denominated.wire_bytes, PACKET_LIMIT),
        (1049, 1232),
        "the Trading common-Hot Denominate frame moved",
    );
    assert!(
        denominated.wire_bytes <= PACKET_LIMIT,
        "the common-Hot Denominate frame is submittable at the K = 3 campaign basis",
    );
    let after_denominate = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after_denominate.replay), 2);
    assert_eq!(lbv2_revision(&after_denominate.aggregate.data), 1);
    assert_eq!(lbv2_revision(&after_denominate.actor_position.data), 1);
    assert_eq!(
        lbv2_position_quantity(&after_denominate.actor_position.data, WINNER),
        ACTOR_CLAIMS[WINNERS] - 1
    );
    assert_eq!(lbv2_revision(&after_denominate.positions[WINNERS].data), 1);
    assert_eq!(
        lbv2_position_quantity(&after_denominate.positions[WINNERS].data, WINNER),
        CUSTODY_CLAIMS[WINNERS] + 1
    );
    assert_eq!(
        mint_supply(&after_denominate.shard_mints[WINNERS]),
        shard_supply(WINNERS) + DENOMINATOR
    );
    assert_eq!(
        token_amount(&after_denominate.actor_shards[WINNERS]),
        actor_shards_after_issue()[WINNERS] + DENOMINATOR
    );
    assert_eq!(
        token_amount(&after_denominate.structured_shards[WINNERS]),
        structured_shards_after_issue()[WINNERS]
    );
    eprintln!(
        "common-Hot Rational: issue accounts={} wire={} CU={} ALT-CU={issue_alt_cu:?}; denominate accounts={} wire={} CU={} ALT-CU={denominate_alt_cu:?}; replay=0→1→2; aggregate=0→1",
        issue.accounts.len(),
        issued.wire_bytes,
        issued.compute_units,
        denominate.accounts.len(),
        denominated.wire_bytes,
        denominated.compute_units,
    );
}

/// THE TERMINAL REDEMPTION, EXECUTING ON THE HOT ROUTE.
///
/// Every other terminal test in this island drives the direct-caller path AND
/// hand-builds its request, so until this one a redemption had never crossed
/// Trading and had never been constructed by the operator at all. The operator
/// builds it -- a 50-account Claims child, planned through the same public
/// planner the browser would use -- and the last wall was one account of
/// artifact geometry.
///
/// `RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3` said the terminal Claims frame
/// was 49 while `REPRESENTATION_FRAME_SPEC_V2`, which owns the frame, said 50.
/// The missing account was the RESOLUTION PROGRAM at the fifth slot of the
/// fourteen-account terminal suffix, so the profile was not merely one rule
/// short: it placed the Custody replay, the Hoard and the recipient one index
/// low and declared no coordinate executable where the Resolution program
/// stands. Both the width and the ORDER now derive from the request contract
/// (`hot_account_profile_v3::declared`), and this test is the executing proof.
///
/// One wall earlier this test refused `InvalidTerminal("custody-replay-no-open-vault")`,
/// a host-side requirement the chain does not hold; that mirror is deleted and
/// `real_sbf_terminal_hostile_joins_and_late_child_failure_are_atomic` carries
/// the proof.
#[tokio::test]
async fn current_common_hot_terminal_redemption_executes_through_real_elves() {
    let trading = trading_artifact();
    let (test, fixture) = fixture_with_basis_and_trading(
        TerminalV1::Provider,
        ReceiptMintRoles::Both,
        ProductBasisProfileV1::Categorical,
        Some(trading),
        CollateralAdapterSelectionV1::Cohort13ZeroExtension,
    );
    let mut context = test.start_with_context().await;

    // NOTHING PLANTS THE REPLAY. Every terminal test in this island starts by
    // executing the real Claims replay-creation route against the real ELFs,
    // because a Claims-role `CustodyReplayV1` is a prestate no other route can
    // produce. The Hot route is no different, and its absence was this test's
    // first wall: `InvalidTerminal("custody-replay-decode")`.
    let created = create_claims_custody_replay(&mut context, &fixture).await;
    assert!(created.accepted, "the real replay-creation route commits");

    // THE OPERATOR BUILDS IT. This is the line that was `NotBearer`, then
    // `InvalidAction`, then `InvalidTerminal("custody-replay-no-open-vault")`.
    let plan = common_hot_open::plan_redeem(&mut context, &fixture).await;

    let profile = dclutch_account_profile_contract::v2::AccountProfileV2::decode(
        &fixture
            .hot_release
            .as_ref()
            .expect("Hot release")
            .redeem
            .account_profile,
    )
    .expect("terminal AccountProfile");
    assert_eq!(
        (
            plan.claims_child.instruction.accounts.len(),
            profile
                .logical_account_count_with_dynamic_spans(0, &[])
                .expect("terminal logical width"),
        ),
        (
            RATIONAL_BASE_ACCOUNT_COUNT_V2
                + RATIONAL_ASSET_ACCOUNT_COUNT_V2
                + RATIONAL_TERMINAL_ACCOUNT_COUNT_V2,
            dclutch_bearer_v2_operator::RATIONAL_TERMINAL_LOGICAL_ACCOUNT_COUNT_V3 as usize,
        ),
        "the terminal Claims child and its release profile are the frame the request contract specifies",
    );

    let bundle = common_hot_open::build_hot(
        &mut context,
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        &plan.family_request,
        &plan.claims_child,
    )
    .await;
    common_hot_open::assert_public_redeem_outer(&mut context, &fixture, &plan, &bundle).await;
    common_hot_open::install(&mut context, &bundle);

    let before = snapshot(&mut context, &fixture).await;
    let redeem = bundle.hot_instruction.clone();
    let addresses = lookup_addresses(
        context.payer.pubkey(),
        fixture.actor.pubkey(),
        std::slice::from_ref(&redeem),
    );
    let (table, alt_cu) = create_live_lookup_table(
        &mut context,
        &addresses,
        "Trading common-Hot Rational RedeemTerminal",
    )
    .await;
    let redeemed = submit_v0_with_heap(
        &mut context,
        &fixture,
        redeem,
        table,
        &addresses,
        "Trading common-Hot Rational RedeemTerminal commits",
    )
    .await
    .expect("real Trading RedeemTerminal transaction");
    if !redeemed.accepted {
        eprintln!(
            "common-Hot RedeemTerminal refusal:\n{}",
            redeemed.logs.join("\n")
        );
    }
    assert!(
        redeemed.accepted,
        "real Trading \u{2192} Claims RedeemTerminal"
    );
    assert!(
        redeemed
            .logs
            .iter()
            .any(|line| line == &format!("Program {CLAIMS_PROGRAM_ID} success")),
        "real Claims ELF must return through Trading"
    );
    assert!(
        redeemed
            .logs
            .iter()
            .any(|line| line == &format!("Program {CUSTODY_PROGRAM_ID} success")),
        "real Custody ELF must move the collateral"
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(
        replay_revision(&after.replay),
        1,
        "one committed redemption"
    );
    eprintln!(
        "common-Hot Rational terminal: accounts={} wire={} CU={} ALT-CU={alt_cu:?}",
        bundle.hot_instruction.accounts.len(),
        redeemed.wire_bytes,
        redeemed.compute_units,
    );
}

async fn account_of(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account query")
        .unwrap_or_else(|| panic!("no account at {key}"))
}

/// One representation coordinate's cost on the wire.
///
/// `ASSET_BYTES_V3` of request tail, plus two bytes per materialized account:
/// one index in the instruction's account-index array and one index in the v0
/// message's lookup list. Nothing compresses either.
const PER_COORDINATE_WIRE_BYTES: usize = ASSET_BYTES_V3 + 2 * RATIONAL_ASSET_ACCOUNT_COUNT_V2;

/// THE PACKET WALL, measured on the executing route, and it is not §3b's wall.
///
/// Decision 0011 §3b measured the executable ceiling at `K = 3` and called it
/// hard. That bound is the **RequestProfile artifact**: `29 + 8K` operations at
/// 24 bytes against `REQUEST_PROFILE_MAX_BYTES_V1 = 1312`, so `K = 3` fits with
/// 8 bytes to spare and `K = 4` is refused at 1,496. It is real, it is not the
/// binding one, and nothing in the tree had measured the other one, because
/// this route had never been submitted at `K = 3`.
///
/// A transaction carrying `IssueStructured` or `UnwrapStructured` at `K = 3` is
/// **1,397 bytes** as a v0 message against a live Address Lookup Table, against
/// a 1,232-byte cluster packet limit. The ALT is already applied — it is what
/// takes the same frame from 2,634 down to 1,397 — so there is no second
/// compression left to reach for. (It was 1,357 against 2,594 until `7b80869d`
/// made every wire measurement carry the `set_compute_unit_limit` a real
/// transaction always pays: +40 bytes on every frame, and the derived `K = 2`
/// ceiling unchanged.)
///
/// The selected-outcome actions are untouched: `Denominate`, `Reconstitute` and
/// `RedeemTerminal` carry `asset_count == 1` at every `K`
/// (`request.rs:470-481`), so their request is `488 + 160` at any width and
/// their frame fits. **So a `K = 3` Product can be denominated, reconstituted
/// and redeemed on a cluster, and can never be issued or unwrapped there.**
///
/// The ceiling is DERIVED here rather than asserted, from two measurements of
/// the same fixture and one constant, so the number moves when the frame does.
#[test]
fn the_full_width_structured_frame_now_fits_a_packet_at_k_three() {
    let (_test, fixture) = fixture(false);
    let payer = Pubkey::new_from_array([0x90; 32]);
    let table = Pubkey::new_from_array([0x91; 32]);
    let full_width = wrapper_instruction(
        &fixture,
        RepresentationActionV2::IssueStructured,
        0,
        false,
        None,
        None,
    );
    let selected = wrapper_instruction(
        &fixture,
        RepresentationActionV2::Denominate,
        2,
        false,
        None,
        None,
    );
    let addresses = lookup_addresses(
        payer,
        fixture.actor.pubkey(),
        &[full_width.clone(), selected.clone()],
    );
    let blockhash = Hash::default();
    let full_bytes =
        live_lookup_v0_wire_bytes(payer, full_width.clone(), blockhash, table, &addresses);
    let selected_bytes =
        live_lookup_v0_wire_bytes(payer, selected.clone(), blockhash, table, &addresses);

    // The published width, restated by the encoder rather than by §3b's prose.
    assert_eq!(
        full_width.data.len(),
        1 + REQUEST_STRUCTURED_HEADER_BYTES_V3 + K * ASSET_BYTES_V3
    );
    assert_eq!(
        selected.data.len(),
        1 + REQUEST_SELECTED_HEADER_BYTES_V3 + ASSET_BYTES_V3
    );
    // THE CLASS HEADER DELTA, which is new and is why the old form of this
    // assertion was four bytes short.
    //
    // It read `full_bytes - selected_bytes == (K - 1) * PER_COORDINATE_WIRE_BYTES`
    // and was exact for two years, because version two gave every action ONE
    // 488-byte header and the only thing separating a full-width frame from a
    // selected one was its extra coordinates. Physical ABI v3 made the header
    // action-conditional, so these two frames no longer share a header: the
    // structured class carries a 32-byte receipt Account where the selected
    // class carries three revisions and an outcome, and it is four bytes wider.
    // The difference is now a SUM of two terms, and reading it as one term made
    // a coordinate look like it cost 74 bytes when it costs 72.
    const CLASS_HEADER_DELTA: usize =
        REQUEST_STRUCTURED_HEADER_BYTES_V3 - REQUEST_SELECTED_HEADER_BYTES_V3;
    assert_eq!(
        full_bytes - selected_bytes,
        CLASS_HEADER_DELTA + (K - 1) * PER_COORDINATE_WIRE_BYTES,
        "a full-width frame costs its class header delta plus one coordinate's \
         asset row and four account indexes for each coordinate past the first"
    );
    assert!(
        selected_bytes <= PACKET_LIMIT,
        "the selected-outcome frame fits at every K: {selected_bytes} bytes"
    );
    assert!(
        full_bytes <= PACKET_LIMIT,
        "the K = {K} full-width frame is measured at {full_bytes} bytes, over the packet limit"
    );
    // The largest full-width K a cluster could actually carry.
    //
    // The baseline is a STRUCTURED frame at K = 1, not the selected frame: the
    // selected frame is a different header class and is the wrong zero for this
    // arithmetic by exactly `CLASS_HEADER_DELTA`.
    let structured_k1_bytes = selected_bytes + CLASS_HEADER_DELTA;
    let executable_full_width_k =
        1 + (PACKET_LIMIT - structured_k1_bytes) / PER_COORDINATE_WIRE_BYTES;
    // SIX, and it was TWO. Every term of that move is named: the base fell from
    // 34 operations to 22 and the row from six to five when the header became
    // action-conditional and the three re-derived keys left the asset row, and
    // the frame fell from 1,397 bytes to under the limit with them.
    assert_eq!(
        executable_full_width_k, 6,
        "the packet ceiling on IssueStructured/UnwrapStructured moved"
    );
    // WHICH WALL BINDS, as a checked fact rather than a sentence.
    //
    // It used to be the packet, one coordinate BELOW the RequestProfile ceiling
    // decision 0011 s3b called hard, and that is why widening the artifact bound
    // did not pay: it would have admitted descriptors that could be published
    // and denominated but never issued. THAT IS NO LONGER TRUE. The packet now
    // admits six coordinates, the artifact admits six, and the binding number is
    // the Structured child ceiling of three -- which is neither of them, and is
    // PROVISIONAL rather than derived.
    //
    // So the lift the cliff doctrine chartered is now worth costing, and what it
    // costs is a campaign run at K = 4 and K = 5, not a constant edit: fitting
    // the packet and the RequestProfile is necessary and not sufficient, because
    // a wider K also costs compute units that only a run can measure.
    let artifact_ceiling_k = usize::try_from(
        dclutch_bearer_v2_operator::RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3,
    )
    .expect("artifact ceiling");
    let child_ceiling_k =
        usize::try_from(STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2).expect("child ceiling");
    assert_eq!(
        executable_full_width_k, artifact_ceiling_k,
        "the packet and the artifact now cap full-width issuance at the same K; if they \
         diverge again, whichever is lower is the one a lift has to move first"
    );
    assert!(
        child_ceiling_k < executable_full_width_k,
        "the Structured child ceiling ({child_ceiling_k}) is the binding one against a packet \
         and artifact that both admit {executable_full_width_k}; it is PROVISIONAL, and the \
         plan for lifting it is a K = 4 and K = 5 campaign run"
    );
    eprintln!(
        "Rational V2 K={K} packet wall: full-width-v0-live-ALT={full_bytes}, \
selected-v0-live-ALT={selected_bytes}, limit={PACKET_LIMIT}, under-by={}, \
per-coordinate={PER_COORDINATE_WIRE_BYTES}, executable-full-width-K={executable_full_width_k}, \
request-profile-ceiling-K={artifact_ceiling_k}",
        PACKET_LIMIT - full_bytes,
    );
}

/// Build the wrapper instruction for request bytes the campaign MUTATED.
///
/// The Trading caller authority is derived from the request digest
/// (`CallerAuthoritySeedsV1`), so a mutated request names a different PDA and
/// the wrapper signs that one. Everything else in the frame is the canonical
/// account list, which is what makes these hostiles hostile: only the bytes
/// moved.
fn wrapper_instruction_from_request(
    fixture: &Fixture,
    action: RepresentationActionV2,
    representation_revision: u64,
    request: &[u8],
) -> Instruction {
    let mut accounts = vec![AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false)];
    accounts.extend(claims_accounts_for_selected(
        fixture,
        action,
        representation_revision,
        WINNER,
        None,
        None,
    ));
    *accounts.get_mut(1).expect("caller authority meta") = AccountMeta::new_readonly(
        caller_authority_for_digest(
            request,
            fixture.market,
            fixture.release_set,
            fixture.parent_context,
        ),
        false,
    );
    let mut data = Vec::with_capacity(request.len() + 1);
    data.push(0_u8);
    data.extend_from_slice(request);
    Instruction {
        program_id: TEST_CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

/// One coordinate's backing skewed by exactly one shard atom.
///
/// `K_i = S * c_i` is recomputed by the callee as
/// `asset.coefficient * header.quantity` (`plan.rs:263`), and the campaign basis
/// is pairwise coprime and coprime to the denominator precisely so a one-atom
/// skew at coordinate `i` cannot be presented as a legitimate quantity at any
/// other coordinate. It has to fail, visibly, at exactly one `i`.
fn one_atom_skew_request(fixture: &Fixture, coordinate: usize) -> Vec<u8> {
    let mut request = request_bytes(fixture, RepresentationActionV2::IssueStructured, 0);
    let skewed = COEFFICIENTS
        .get(coordinate)
        .copied()
        .expect("coefficient")
        .checked_add(1)
        .expect("one-atom skew");
    put_u64(
        &mut request,
        REQUEST_STRUCTURED_HEADER_BYTES_V3
            + coordinate * ASSET_BYTES_V3
            + ASSET_COEFFICIENT_OFFSET_V3,
        skewed,
    );
    request
}

/// A coordinate backed by the RECEIPT Mint: a receipt backed by itself.
///
/// MOVED BY physical ABI v3, not retired. The hostile used to overwrite row
/// zero's inlined shard Mint with the receipt Mint and was refused by
/// `RepresentationRequestV2::validate`'s alias check. The Mint is DERIVED from
/// `(program_id, descriptor_id, outcome)` now, so a coordinate cannot name the
/// receipt Mint on the wire at all and these bytes are honest: the request
/// below is the canonical `IssueStructured`, byte for byte.
///
/// The substitution happens one layer down, in the ACCOUNT FRAME, which is
/// where the property lives now: the caller hands coordinate zero the receipt
/// Mint account and `authenticate_asset_identities` refuses
/// `ClaimsSbfError::ReceiptAlias` by name, before deriving anything. Its twin
/// over the derived keys is `ResolvedRequestV2::join`, and its twin over the
/// terms is `bind_shard_terms`; this test asserts all three.
fn receipt_backed_by_receipt_request(fixture: &Fixture) -> Vec<u8> {
    request_bytes(fixture, RepresentationActionV2::IssueStructured, 0)
}

/// THE STRUCTURED FAMILY HOSTILES, every one of them through the real wire.
///
/// Each is submitted as a real transaction against the real ELFs, must refuse,
/// and must leave the complete resource snapshot byte-identical to its
/// prestate. A hostile that refuses while moving one lamport is not a refusal.
#[tokio::test]
async fn the_structured_family_hostiles_refuse_through_the_real_wire() {
    let (test, fixture) = fixture(false);
    let mut context = test.start_with_context().await;

    // FOUNDING-TIME, before a byte reaches a chain: the rank rule. A receipt can
    // never be backed by itself, so a lowering whose receipt Mint aliases a
    // shard Mint cannot produce terms at all.
    assert_eq!(
        structured_lowering::decode_terms_with_receipt_aliasing_a_shard_mint(&fixture.basis).err(),
        Some(dclutch_structured_v2_kernel::Error::DuplicateIdentity),
        "a receipt Mint that aliases a shard Mint must not survive bind_shard_terms"
    );

    // DIRECTION ONE: the exposure record where the descriptor belongs.
    let exposure_as_descriptor = wrapper_instruction(
        &fixture,
        RepresentationActionV2::IssueStructured,
        0,
        false,
        Some((fixture.graph_raw, fixture.graph_staging)),
        None,
    );
    // DIRECTION TWO: the descriptor record where the exposure belongs.
    let descriptor_as_exposure = wrapper_instruction(
        &fixture,
        RepresentationActionV2::IssueStructured,
        0,
        false,
        None,
        Some((fixture.descriptor_raw, fixture.descriptor_staging)),
    );
    // A same-width descriptor whose recipe is the canonical one PERMUTED.
    let permuted_recipe = wrapper_instruction(
        &fixture,
        RepresentationActionV2::IssueStructured,
        0,
        false,
        Some((
            fixture.alternate_descriptor_raw,
            fixture.alternate_descriptor_staging,
        )),
        None,
    );
    let receipt_backed = {
        let request = receipt_backed_by_receipt_request(&fixture);
        // THE OWNER OF THIS REFUSAL MOVED, and the campaign now names the new
        // one. The request grammar used to refuse these bytes outright -- a
        // shard Mint may not alias the receipt Mint -- because the shard Mint
        // rode the wire and `decode` could compare the two. Physical ABI v3
        // takes it OFF the wire and has the Claims adapter derive it, so these
        // bytes decode clean and the substitution has to be made where the
        // account actually arrives: the METAS.
        assert!(
            RepresentationRequestV2::decode(&request).is_ok(),
            "v3 leaves the grammar nothing to compare: the shard Mint is derived, not sent"
        );
        let mut instruction = wrapper_instruction_from_request(
            &fixture,
            RepresentationActionV2::IssueStructured,
            0,
            &request,
        );
        let coordinate = fixture.assets.first().expect("first asset").mint;
        assert_ne!(
            coordinate, fixture.receipt_mint,
            "the fixture must not already alias what this hostile substitutes"
        );
        let mut substituted = 0_usize;
        for meta in &mut instruction.accounts {
            if meta.pubkey == coordinate {
                meta.pubkey = fixture.receipt_mint;
                substituted += 1;
            }
        }
        // A hostile that substituted nothing is an honest transaction wearing a
        // hostile label, and it would COMMIT while this loop reported success.
        assert_eq!(
            substituted, 1,
            "coordinate zero's Mint appears exactly once in the Claims frame"
        );
        instruction
    };
    let skews: Vec<Instruction> = (0..K)
        .map(|coordinate| {
            wrapper_instruction_from_request(
                &fixture,
                RepresentationActionV2::IssueStructured,
                0,
                &one_atom_skew_request(&fixture, coordinate),
            )
        })
        .collect();

    let payer = context.payer.pubkey();
    let mut instructions = vec![
        exposure_as_descriptor.clone(),
        descriptor_as_exposure.clone(),
        permuted_recipe.clone(),
        receipt_backed.clone(),
    ];
    instructions.extend(skews.iter().cloned());
    let addresses = lookup_addresses(payer, fixture.actor.pubkey(), &instructions);
    let (table, _) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: family hostiles",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    // Every row names the exact discriminant it must refuse with. A bare
    // `is_err` here would pass on whatever the transaction reaches first, and
    // this list is exactly the set of hostiles whose refusal moved once
    // already.
    let mut labelled = vec![
        (
            "the finalized exposure record in the descriptor's slot",
            "cross-schema substitution: exposure as descriptor",
            ClaimsSbfError::Identity,
            exposure_as_descriptor,
        ),
        (
            "the immutable descriptor in the exposure record's slot",
            "cross-schema substitution: descriptor as exposure",
            ClaimsSbfError::Identity,
            descriptor_as_exposure,
        ),
        (
            "a same-width descriptor whose coefficients are the canonical ones permuted",
            "recipe substitution: permuted coefficients",
            ClaimsSbfError::Identity,
            permuted_recipe,
        ),
        (
            "a coordinate backed by the receipt Mint itself",
            "receipt backed by receipt",
            ClaimsSbfError::ReceiptAlias,
            receipt_backed,
        ),
    ];
    for (coordinate, instruction) in skews.into_iter().enumerate() {
        labelled.push((
            "one shard atom of backing skew at a single coordinate",
            match coordinate {
                0 => "one-atom K_i skew at coordinate 0",
                1 => "one-atom K_i skew at coordinate 1",
                _ => "one-atom K_i skew at coordinate 2",
            },
            ClaimsSbfError::Representation,
            instruction,
        ));
    }

    for (why, label, code, instruction) in labelled {
        let result = submit_v0(
            &mut context,
            &fixture,
            instruction,
            table,
            &addresses,
            &format!("claims rational-representation-v2: {label}"),
        )
        .await
        .expect("hostile transaction");
        assert_refused_with(&result, code as u32, why);
        assert_eq!(
            snapshot(&mut context, &fixture).await,
            before,
            "{why} must leave every resource byte-identical"
        );
        eprintln!(
            "Rational V2 hostile: {label} refused {:#x} at {} CU, v0={} bytes",
            code as u32, result.compute_units, result.wire_bytes,
        );
    }
}

/// §3b's TWO Token-2022 roles, made executable — and the ruling's failure mode
/// does not happen.
///
/// The receipt Mint here carries the representation authority as its Mint
/// authority and NOT as its permissioned-burn authority: 0011 §3b's
/// under-configured founding. The ruling predicted that `IssueStructured` would
/// commit (`mint_to_checked` needs only the first role) and `BurnReceipt` would
/// then fail "at the Token program with the descriptor already committed",
/// stranding outstanding receipts against a Mint that can never burn them.
///
/// MEASURED: that cannot happen. `parse_behavior_mint` reads the receipt Mint's
/// Token-2022 behavior profile on EVERY action and requires the permissioned-burn
/// authority to be present and pinned to the representation authority, so the
/// FIRST `IssueStructured` refuses `ClaimsSbfError::Token` (0x5009) before any
/// Token-2022 CPI is issued at all. The two-role requirement is enforced by the
/// adapter up front, not discovered at unwrap time. §3b's cost is real —
/// founding must configure both roles — but its consequence is a founding that
/// can never issue, not a representation that can never be unwound.
///
/// A correction this test earned the hard way, recorded because the inference
/// drawn from it was wrong. The 202 bytes below were not a hypothetical
/// under-configured founding: until the writer was fixed they were EXACTLY what
/// `rational_lifecycle_v2::initialize_closeable_mint` allocated and
/// initialized, which makes this an executable proof that the founding path the
/// protocol shipped could never issue — filed here as reassurance, because the
/// two campaigns are disjoint and nothing ever handed one route's output to the
/// other's reader. The lifecycle now writes both roles at 238 bytes and its own
/// campaign asserts that against this very reader. So this stays a hostile, and
/// it is finally only a hostile.
#[tokio::test]
async fn a_receipt_mint_missing_its_burn_role_refuses_at_the_first_issue() {
    let (test, fixture) = fixture_with(TerminalV1::None, ReceiptMintRoles::MintAuthorityOnly);
    let mut context = test.start_with_context().await;
    let issue = wrapper_instruction(
        &fixture,
        RepresentationActionV2::IssueStructured,
        0,
        false,
        None,
        None,
    );
    let unwrap = wrapper_instruction(
        &fixture,
        RepresentationActionV2::UnwrapStructured,
        1,
        false,
        None,
        None,
    );
    let payer = context.payer.pubkey();
    let addresses = lookup_addresses(
        payer,
        fixture.actor.pubkey(),
        &[issue.clone(), unwrap.clone()],
    );
    let (table, _) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: single-role receipt Mint",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    let issued = submit_v0(
        &mut context,
        &fixture,
        issue,
        table,
        &addresses,
        "claims rational-representation-v2: issue against a receipt Mint with only the Mint role",
    )
    .await
    .expect("IssueStructured transaction");
    assert!(
        !issued.accepted,
        "a receipt Mint missing its burn role must not be issued against"
    );
    assert!(
        issued.logs.iter().any(|log| log
            == &format!("Program {CLAIMS_PROGRAM_ID} failed: custom program error: 0x5009")),
        "the refusal must be ClaimsSbfError::Token: {}",
        issued.logs.join("\n")
    );
    assert!(
        !issued
            .logs
            .iter()
            .any(|log| log.starts_with(&format!("Program {TOKEN_PROGRAM_ID} invoke"))),
        "the adapter must refuse BEFORE any Token-2022 CPI, so no receipt is ever \
         minted against a Mint that could not burn it"
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before,
        "nothing may commit against an under-configured receipt Mint"
    );

    let unwrapped = submit_v0(
        &mut context,
        &fixture,
        unwrap,
        table,
        &addresses,
        "claims rational-representation-v2: unwrap against a receipt Mint with only the Mint role",
    )
    .await
    .expect("UnwrapStructured transaction");
    // Unwrap refuses too, and NOT for the burn role: the Issue above committed
    // nothing, so no representation replay exists to carry revision one and the
    // frame is refused on accounts (0x5001). That is the whole consequence of the
    // adapter catching the missing role up front -- there is never anything to
    // unwrap, so `BurnReceipt` is never the thing that fails.
    assert!(
        !unwrapped.accepted,
        "there is nothing to unwrap against a founding that could not issue"
    );
    assert!(
        unwrapped.logs.iter().any(|log| log
            == &format!("Program {CLAIMS_PROGRAM_ID} failed: custom program error: 0x5001")),
        "the unwrap refusal is the missing replay, not the missing burn role: {}",
        unwrapped.logs.join("\n")
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before,
        "the refused unwrap must leave every resource byte-identical"
    );
    eprintln!(
        "Rational V2 two-Token-2022-roles: issue-refusal-CU={}, unwrap-refusal-CU={}, \
Token-2022 CPIs issued=0",
        issued.compute_units, unwrapped.compute_units,
    );
}

#[tokio::test]
async fn real_sbf_terminal_hostile_joins_and_late_child_failure_are_atomic() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    // The Claims-role replay is CREATED, not planted. Everything below stands on
    // an account the tree produced.
    let created = create_claims_custody_replay(&mut context, &fixture).await;
    assert!(
        created.accepted,
        "the Claims-role Custody replay must be creatable: {}",
        created.logs.join("\n")
    );
    assert!(
        created.wire_bytes <= PACKET_LIMIT,
        "replay creation must fit a legacy packet without an ALT: {} bytes",
        created.wire_bytes
    );
    eprintln!(
        "Claims-role Custody replay created: legacy={} bytes (limit {PACKET_LIMIT}), CU={}",
        created.wire_bytes, created.compute_units,
    );
    let positive = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        None,
        None,
    );
    let late = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        true,
        None,
        None,
    );
    let descriptor_substitution = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        Some((
            fixture.alternate_descriptor_raw,
            fixture.alternate_descriptor_staging,
        )),
        None,
    );
    let graph_substitution = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        None,
        Some((fixture.alternate_graph_raw, fixture.alternate_graph_staging)),
    );
    let mut certificate_substitution = positive.clone();
    let certificate_meta = 1 + RATIONAL_BASE_ACCOUNT_COUNT_V2 + RATIONAL_ASSET_ACCOUNT_COUNT_V2 + 3;
    certificate_substitution
        .accounts
        .get_mut(certificate_meta)
        .expect("terminal certificate meta")
        .pubkey = sysvar::rent::ID;
    let expected_claims_accounts = RATIONAL_BASE_ACCOUNT_COUNT_V2
        + RATIONAL_ASSET_ACCOUNT_COUNT_V2
        + RATIONAL_TERMINAL_ACCOUNT_COUNT_V2;
    assert_eq!(positive.accounts.len(), 1 + expected_claims_accounts);
    assert_eq!(
        positive.data.len(),
        1 + REQUEST_TERMINAL_HEADER_BYTES_V3 + ASSET_BYTES_V3
    );
    let payer = context.payer.pubkey();
    let instructions = [
        positive.clone(),
        late.clone(),
        descriptor_substitution.clone(),
        graph_substitution.clone(),
        certificate_substitution.clone(),
    ];
    let addresses = lookup_addresses(payer, fixture.actor.pubkey(), &instructions);
    let (table, lookup_cu) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: terminal",
    )
    .await;
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("post-ALT blockhash");
    let (legacy, no_alt, live_alt) =
        packet_measurements(payer, &positive, blockhash, table, &addresses);
    eprintln!(
        "Rational V2 terminal packet preflight: request={}, claims-frame={}, outer-metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, ALT-CU={lookup_cu:?}",
        REQUEST_TERMINAL_HEADER_BYTES_V3 + ASSET_BYTES_V3,
        expected_claims_accounts,
        positive.accounts.len(),
        unique_account_count(&positive),
        legacy,
        no_alt,
        live_alt,
    );
    let before = snapshot(&mut context, &fixture).await;
    // THE CONJUNCT THE CHAIN DOES NOT HOLD, asserted where the chain settles it.
    //
    // The positive redemption below COMMITS -- Hoard down one atom, recipient up
    // one, replay revision advanced -- against a Claims-role Custody replay this
    // test created through the real route moments ago, and that replay counts
    // ZERO open vaults. `create_claims_custody_replay` mints it that way and
    // `the_claims_role_replay_is_created_exactly_once` asserts it.
    //
    // So Custody executes a positive terminal payout at `open_vault_count == 0`,
    // and the host-side operator's matching requirement was a mirror of a
    // conjunct no on-chain route holds. It is deleted in this change; this line
    // is what refutes it, and it runs on real ELFs.
    assert_eq!(
        CustodyReplayV1::decode(&before.custody_replay.as_ref().expect("Custody replay").data)
            .expect("pre-redemption Custody replay")
            .open_vault_count,
        0,
        "the chain pays a positive terminal redemption with no open vault on the Claims-role replay",
    );

    for (label, hostile) in [
        ("same-width descriptor", descriptor_substitution),
        ("same-width graph", graph_substitution),
        (
            "substituted Resolution certificate",
            certificate_substitution,
        ),
    ] {
        let result = submit_v0(
            &mut context,
            &fixture,
            hostile,
            table,
            &addresses,
            &format!("claims rational-representation-v2: terminal against a {label}"),
        )
        .await
        .expect("hostile substitution transaction");
        assert!(!result.accepted, "{label} substitution must refuse");
        assert_eq!(
            snapshot(&mut context, &fixture).await,
            before,
            "{label} refusal must be byte-exact rollback"
        );
    }

    let late_result = submit_v0(
        &mut context,
        &fixture,
        late,
        table,
        &addresses,
        "claims rational-representation-v2: caller refuses after a complete terminal redemption",
    )
    .await
    .expect("late rollback transaction");
    if !late_result.accepted {
        eprintln!(
            "Terminal late-refusal logs:\n{}",
            late_result.logs.join("\n")
        );
    }
    assert!(
        !late_result.accepted,
        "late wrapper must deliberately refuse"
    );
    assert!(
        late_result.wire_bytes <= PACKET_LIMIT,
        "late packet overflow"
    );
    assert!(
        late_result
            .logs
            .iter()
            .any(|log| log == &format!("Program {CUSTODY_PROGRAM_ID} success")),
        "real Custody must return before the late refusal"
    );
    assert!(
        late_result
            .logs
            .iter()
            .any(|log| log == &format!("Program {CLAIMS_PROGRAM_ID} success")),
        "real Claims must return before the late refusal"
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        before,
        "late refusal must roll back rational replay, Claims, Token-2022, and Custody"
    );

    let accepted = submit_v0(
        &mut context,
        &fixture,
        positive.clone(),
        table,
        &addresses,
        "claims rational-representation-v2: winning terminal redemption commits",
    )
    .await
    .expect("positive terminal transaction");
    assert!(accepted.accepted, "terminal composition must commit");
    assert!(
        accepted.wire_bytes <= PACKET_LIMIT,
        "terminal packet overflow"
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after.replay), 1);
    assert_eq!(lbv2_revision(&after.aggregate.data), 1);
    assert_eq!(
        lbv2_revision(&after.positions.first().expect("first Position").data),
        0
    );
    assert_eq!(
        lbv2_revision(&after.positions.get(WINNERS).expect("winner Position").data),
        1
    );
    // The redemption burns one whole claim's worth of raw shards from the actor
    // and pays one collateral atom. Structured custody is untouched: the receipts
    // it backs are still outstanding.
    assert_eq!(
        lbv2_position_quantity(
            &after.positions.get(WINNERS).expect("winner Position").data,
            WINNER,
        ),
        CUSTODY_CLAIMS[WINNERS] - 1
    );
    assert_eq!(
        mint_supply(after.shard_mints.get(WINNERS).expect("winner Mint")),
        shard_supply(WINNERS) - DENOMINATOR
    );
    assert_eq!(
        token_amount(
            after
                .actor_shards
                .get(WINNERS)
                .expect("winner actor shards")
        ),
        actor_shards()[WINNERS] - DENOMINATOR
    );
    assert_eq!(
        token_amount(
            after
                .structured_shards
                .get(WINNERS)
                .expect("winner structured shards"),
        ),
        structured_shards()[WINNERS]
    );
    assert_eq!(
        token_amount(
            after
                .actor_shards
                .get(WINNERS)
                .expect("winner actor shards")
        ) + token_amount(
            after
                .structured_shards
                .get(WINNERS)
                .expect("winner structured shards")
        ),
        mint_supply(after.shard_mints.get(WINNERS).expect("winner Mint"))
    );
    assert_eq!(mint_supply(&after.receipt_mint), RECEIPT_SUPPLY);

    // DOUBLE REDEEM. The identical request, resubmitted. It is a genuinely
    // different transaction -- a later slot means a different blockhash and
    // therefore a different signature, so the runtime's own duplicate-signature
    // dedup is NOT what refuses it. The assertion that it is the protocol is the
    // log: Claims itself must fail, having already consumed the representation
    // replay's revision and the Custody replay's cursor.
    let redeemed_slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-redemption Clock")
        .slot;
    context
        .warp_to_slot(redeemed_slot + 1)
        .expect("a later slot for the replayed redemption");
    let replayed = submit_v0(
        &mut context,
        &fixture,
        positive.clone(),
        table,
        &addresses,
        "claims rational-representation-v2: the winning terminal redemption replayed",
    )
    .await
    .expect("replayed terminal transaction");
    assert!(!replayed.accepted, "a redemption must not be replayable");
    assert!(
        replayed
            .logs
            .iter()
            .any(|log| log.starts_with(&format!("Program {CLAIMS_PROGRAM_ID} failed"))),
        "the replay must be refused by Claims, not deduplicated by the runtime: {}",
        replayed.logs.join("\n")
    );
    assert_eq!(
        snapshot(&mut context, &fixture).await,
        after,
        "the refused replay must leave the settled state byte-identical"
    );
    eprintln!(
        "Rational V2 double redeem: refused at {} CU",
        replayed.compute_units
    );

    let custody_replay = after.custody_replay.as_ref().expect("Custody replay");
    assert_eq!(
        CustodyReplayV1::decode(&custody_replay.data)
            .expect("post Custody replay")
            .next_revision,
        CUSTODY_EXPECTED_REVISION + 1
    );
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        INITIAL_HOARD_ATOMS - 1,
        "Custody Hoard principal must pay exactly one atom without violating terminal solvency"
    );
    assert_eq!(
        token_amount(after.recipient.as_ref().expect("recipient")),
        INITIAL_RECIPIENT_ATOMS + 1
    );
    eprintln!(
        "Rational V2 terminal: request={}, claims-frame={}, outer-metas={}, unique={}, legacy={}, v0-no-ALT={}, v0-live-ALT={}, positive-v0={}, positive-CU={}, late-v0={}, late-CU={}, ALT-CU={lookup_cu:?}",
        REQUEST_TERMINAL_HEADER_BYTES_V3 + ASSET_BYTES_V3,
        expected_claims_accounts,
        positive.accounts.len(),
        unique_account_count(&positive),
        legacy,
        no_alt,
        live_alt,
        accepted.wire_bytes,
        accepted.compute_units,
        late_result.wire_bytes,
        late_result.compute_units,
    );
}

#[tokio::test]
async fn real_sbf_losing_terminal_burns_raw_shards_without_custody_payout() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    // The Claims-role replay is CREATED, not planted. Everything below stands on
    // an account the tree produced.
    let created = create_claims_custody_replay(&mut context, &fixture).await;
    assert!(
        created.accepted,
        "the Claims-role Custody replay must be creatable: {}",
        created.logs.join("\n")
    );
    assert!(
        created.wire_bytes <= PACKET_LIMIT,
        "replay creation must fit a legacy packet without an ALT: {} bytes",
        created.wire_bytes
    );
    eprintln!(
        "Claims-role Custody replay created: legacy={} bytes (limit {PACKET_LIMIT}), CU={}",
        created.wire_bytes, created.compute_units,
    );
    let losing = wrapper_instruction_for_selected(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        0,
        false,
        None,
        None,
    );
    let payer = context.payer.pubkey();
    let addresses = lookup_addresses(payer, fixture.actor.pubkey(), std::slice::from_ref(&losing));
    let (table, lookup_cu) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: losing terminal",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    let accepted = submit_v0(
        &mut context,
        &fixture,
        losing.clone(),
        table,
        &addresses,
        "claims rational-representation-v2: losing terminal redemption commits",
    )
    .await
    .expect("zero-payout terminal transaction");
    if !accepted.accepted {
        eprintln!(
            "Zero-payout terminal refusal logs:\n{}",
            accepted.logs.join("\n")
        );
    }
    assert!(accepted.accepted, "zero-payout terminal must commit");
    assert!(
        accepted.wire_bytes <= PACKET_LIMIT,
        "zero-payout packet overflow"
    );
    assert!(
        accepted
            .logs
            .iter()
            .any(|log| log == &format!("Program {TOKEN_PROGRAM_ID} success")),
        "real Token-2022 permissioned burn must execute"
    );
    assert!(
        !accepted
            .logs
            .iter()
            .any(|log| log == &format!("Program {CUSTODY_PROGRAM_ID} success")),
        "zero payout must not invoke Custody"
    );

    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after.replay), 1);
    assert_eq!(lbv2_revision(&after.aggregate.data), 1);
    assert_eq!(lbv2_revision(&after.positions[0].data), 1);
    assert_eq!(
        lbv2_position_quantity(&after.positions[0].data, 0),
        CUSTODY_CLAIMS[0] - 1
    );
    for index in 1..K {
        for (actual, expected) in [
            (after.positions.get(index), before.positions.get(index)),
            (after.shard_mints.get(index), before.shard_mints.get(index)),
            (
                after.actor_shards.get(index),
                before.actor_shards.get(index),
            ),
        ] {
            assert_account_content_eq(
                actual.expect("observed coordinate"),
                expected.expect("prestate coordinate"),
            );
        }
    }
    assert_eq!(
        mint_supply(&after.shard_mints[0]),
        shard_supply(0) - DENOMINATOR
    );
    assert_eq!(
        token_amount(&after.actor_shards[0]),
        actor_shards()[0] - DENOMINATOR
    );
    assert_eq!(
        token_amount(&after.structured_shards[0]),
        structured_shards()[0]
    );
    assert_eq!(
        token_amount(&after.actor_shards[0]) + token_amount(&after.structured_shards[0]),
        mint_supply(&after.shard_mints[0]),
        "zero-payout burn must conserve the raw-unit shard remainder exactly"
    );
    assert_account_content_eq(&after.shard_mints[1], &before.shard_mints[1]);
    assert_account_content_eq(&after.actor_shards[1], &before.actor_shards[1]);
    assert_account_content_eq(&after.structured_shards[1], &before.structured_shards[1]);
    assert_account_content_eq(&after.receipt_mint, &before.receipt_mint);
    assert_account_content_eq(&after.actor_receipt, &before.actor_receipt);
    assert_account_content_eq(
        after.custody_replay.as_ref().expect("Custody replay"),
        before.custody_replay.as_ref().expect("pre Custody replay"),
    );
    assert_account_content_eq(
        after.hoard.as_ref().expect("Hoard"),
        before.hoard.as_ref().expect("pre Hoard"),
    );
    assert_account_content_eq(
        after.recipient.as_ref().expect("recipient"),
        before.recipient.as_ref().expect("pre recipient"),
    );
    eprintln!(
        "Rational V2 zero terminal: selected=0, request={}, outer-metas={}, v0={}, CU={}, ALT-CU={lookup_cu:?}",
        REQUEST_TERMINAL_HEADER_BYTES_V3 + ASSET_BYTES_V3,
        losing.accounts.len(),
        accepted.wire_bytes,
        accepted.compute_units,
    );
}

/// The role is a SEED and a CHECKED FIELD, and neither half can be forged past
/// the other.
///
/// Before decision 0008's addendum the two were independent: a replay's address
/// said nothing about its role, so one namespace admitted exactly the first role
/// that arrived and the payout routes' `caller_role != Claims` guard was the
/// only thing standing between a founding's Trading cursor and a redemption.
/// With the role in the seeds, a replay at the Claims address is written by an
/// `InitializeReplay` whose `caller_role` is Claims, so the bytes and the
/// address agree BY CONSTRUCTION.
///
/// This forges the bytes anyway -- writes a Trading-role replay straight into
/// the Claims-role account, which no route could produce -- and requires the
/// redemption to refuse and move nothing. Custody would refuse it too, one
/// layer down, on `ReplayBindingMismatch`.
#[tokio::test]
async fn a_trading_role_replay_never_serves_a_claims_payout() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let created = create_claims_custody_replay(&mut context, &fixture).await;
    assert!(created.accepted, "{}", created.logs.join("\n"));
    let terminal = fixture.terminal_accounts.expect("terminal fixture");

    let honest = observed(&mut context, terminal.custody_replay).await;
    let state = CustodyReplayV1::decode(&honest.data).expect("the created Claims-role replay");
    assert_eq!(state.caller_role, CustodyCallerRoleV1::Claims);
    assert_eq!(state.next_revision, CUSTODY_EXPECTED_REVISION);
    assert_eq!(state.open_vault_count, 0);
    assert_eq!(state.context, fixture.custody_context);
    assert_eq!(state.rent_refund, market_rent_credit().to_bytes());

    let mut forged = honest.clone();
    let bytes = CustodyReplayV1 {
        caller_role: CustodyCallerRoleV1::Trading,
        ..state
    }
    .to_bytes()
    .expect("a Trading-role replay");
    forged.data.copy_from_slice(&bytes);
    assert_ne!(forged.data, honest.data, "a forgery that forges nothing");
    context.set_account(&terminal.custody_replay, &AccountSharedData::from(forged));

    let positive = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        None,
        None,
    );
    let payer = context.payer.pubkey();
    let addresses = lookup_addresses(
        payer,
        fixture.actor.pubkey(),
        std::slice::from_ref(&positive),
    );
    let (table, _) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: cross-role replay",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;
    let result = submit_v0(
        &mut context,
        &fixture,
        positive,
        table,
        &addresses,
        "claims rational-representation-v2: a Trading-role replay at the Claims-role address",
    )
    .await
    .expect("cross-role replay transaction");
    assert!(
        !result.accepted,
        "a Trading-role replay must not pay a Claims redemption: {:#?}",
        result.logs
    );
    assert!(
        result.logs.iter().any(|log| log
            == &format!(
                "Program {CLAIMS_PROGRAM_ID} failed: custom program error: {:#x}",
                ClaimsSbfError::Identity as u32
            )),
        "the refusal must be Claims' own identity join: {:#?}",
        result.logs
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_account_content_eq(
        after.hoard.as_ref().expect("Hoard"),
        before.hoard.as_ref().expect("pre Hoard"),
    );
    assert_account_content_eq(
        after.recipient.as_ref().expect("recipient"),
        before.recipient.as_ref().expect("pre recipient"),
    );
}

/// Creation authenticates by DERIVATION, not by what an account says.
///
/// Two substitutions, both of which would put a Claims-role replay somewhere the
/// namespace's owner never named: an aggregate that is not the one the wire's
/// Market derives, and a replay account that is not the Claims-role address the
/// aggregate's namespace derives. The second is the role-seeded address spoof --
/// the Trading compartment's own address, offered as the place to write a
/// Claims-role cursor.
#[tokio::test]
async fn replay_creation_refuses_a_substituted_aggregate_or_replay() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let trading_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            fixture.market.to_bytes(),
            fixture.release_set,
            CustodyCallerRoleV1::Trading,
            fixture.custody_context,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    assert_ne!(trading_replay, terminal.custody_replay);

    for (overrides, label) in [
        (
            ReplayCreationOverrides {
                aggregate: Some(fixture.market),
                replay: None,
            },
            "the Core Market standing in for the Claims aggregate",
        ),
        (
            ReplayCreationOverrides {
                aggregate: None,
                replay: Some(trading_replay),
            },
            "the Trading-role replay address offered for a Claims-role cursor",
        ),
    ] {
        let result = submit_replay_creation(
            &mut context,
            &fixture,
            overrides,
            &format!("claims rational-representation-v2: replay creation with {label}"),
        )
        .await;
        assert!(!result.accepted, "{label} must refuse: {:#?}", result.logs);
        assert!(
            result.logs.iter().any(|log| log
                == &format!(
                    "Program {CLAIMS_PROGRAM_ID} failed: custom program error: {:#x}",
                    ClaimsSbfError::Identity as u32
                )),
            "{label} must refuse on Claims' identity join: {:#?}",
            result.logs
        );
        assert!(
            context
                .banks_client
                .get_account(terminal.custody_replay)
                .await
                .expect("account query")
                .is_none(),
            "{label} must not have created the canonical replay"
        );
        assert!(
            context
                .banks_client
                .get_account(trading_replay)
                .await
                .expect("account query")
                .is_none(),
            "{label} must not have created the Trading compartment either"
        );
    }
}

/// The cursor is created once and only once.
///
/// Creation is permissionless -- anyone may prepay it -- so the second submitter
/// has to lose harmlessly rather than reset a live cursor. It does: Claims
/// requires the replay account to be vacant and System-owned before it forwards
/// anything, so the second attempt refuses before Custody is reached and the
/// first submitter's rent refund stands.
#[tokio::test]
async fn the_claims_role_replay_is_created_exactly_once() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let first = create_claims_custody_replay(&mut context, &fixture).await;
    assert!(first.accepted, "{}", first.logs.join("\n"));
    let created = observed(&mut context, terminal.custody_replay).await;

    let second = submit_replay_creation(
        &mut context,
        &fixture,
        ReplayCreationOverrides::default(),
        "claims rational-representation-v2: a second replay creation",
    )
    .await;
    assert!(
        !second.accepted,
        "a live replay must not be re-created: {:#?}",
        second.logs
    );
    assert_account_content_eq(
        &observed(&mut context, terminal.custody_replay).await,
        &created,
    );
    assert_eq!(
        CustodyReplayV1::decode(&created.data)
            .expect("live replay")
            .rent_refund,
        market_rent_credit().to_bytes(),
        "the Core-selected lifecycle credit keeps the refund"
    );
}

/// The persisted namespace is the ONLY thing that names the Hoard, so what
/// happens when it is wrong is the whole safety question.
///
/// The Claims aggregate's `custody_context` is now the sole persisted owner of
/// the Market's Custody namespace: `terminal_settlement_v3`,
/// `rational_terminal_v3`, `liability_basis_v2` and the operator all derive the
/// Hoard Vault, the replay and the caller PDA from it and none of them assumes
/// the Market address any more. That makes the field load-bearing, and a
/// load-bearing field earns an adversarial case.
///
/// In every case below the founding's own state is untouched -- the Hoard holds
/// its principal at
/// `SHA-256(PROJECTED_HOARD_CONTEXT_DOMAIN_V1 || FOUNDING_ACTION_CONTEXT_V1)`
/// and the live replay sits beside it -- and only the aggregate's claim about
/// where that is moves. A substituted namespace must refuse and move nothing;
/// it must never reach some other compartment's money.
async fn a_substituted_custody_namespace_refuses(
    substitute: fn(&Fixture) -> [u8; 32],
    label: &str,
) {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    // The Claims-role replay is CREATED, not planted. Everything below stands on
    // an account the tree produced.
    let created = create_claims_custody_replay(&mut context, &fixture).await;
    assert!(
        created.accepted,
        "the Claims-role Custody replay must be creatable: {}",
        created.logs.join("\n")
    );
    assert!(
        created.wire_bytes <= PACKET_LIMIT,
        "replay creation must fit a legacy packet without an ALT: {} bytes",
        created.wire_bytes
    );
    eprintln!(
        "Claims-role Custody replay created: legacy={} bytes (limit {PACKET_LIMIT}), CU={}",
        created.wire_bytes, created.compute_units,
    );
    let substituted = substitute(&fixture);
    let positive = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        None,
        None,
    );
    let payer = context.payer.pubkey();
    let addresses = lookup_addresses(
        payer,
        fixture.actor.pubkey(),
        std::slice::from_ref(&positive),
    );
    let (table, _) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: substituted custody namespace",
    )
    .await;

    let honest = observed(&mut context, fixture.aggregate).await;
    assert_ne!(
        substituted, fixture.custody_context,
        "a substitution that is not a substitution proves nothing"
    );
    let mut lying = honest.clone();
    lying
        .data
        .get_mut(
            LiabilityBasisMarketLayoutV2::CUSTODY_CONTEXT
                ..LiabilityBasisMarketLayoutV2::CUSTODY_CONTEXT + 32,
        )
        .expect("aggregate custody-context field")
        .copy_from_slice(&substituted);
    context.set_account(&fixture.aggregate, &AccountSharedData::from(lying));
    let before = snapshot(&mut context, &fixture).await;

    let result = submit_v0(
        &mut context,
        &fixture,
        positive,
        table,
        &addresses,
        &format!("claims rational-representation-v2: custody namespace substituted with {label}"),
    )
    .await
    .expect("substituted-namespace transaction");
    assert!(
        !result.accepted,
        "a Market that lies about its Custody namespace must not redeem: {:#?}",
        result.logs
    );
    assert!(
        result.logs.iter().any(|log| log
            == &format!(
                "Program {CLAIMS_PROGRAM_ID} failed: custom program error: {:#x}",
                ClaimsSbfError::Identity as u32
            )),
        "the refusal must be Claims' own identity join, not an incidental failure: {:#?}",
        result.logs
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_account_content_eq(
        after.hoard.as_ref().expect("Hoard"),
        before.hoard.as_ref().expect("pre Hoard"),
    );
    assert_account_content_eq(
        after.recipient.as_ref().expect("recipient"),
        before.recipient.as_ref().expect("pre recipient"),
    );
    assert_account_content_eq(
        after.custody_replay.as_ref().expect("Custody replay"),
        before.custody_replay.as_ref().expect("pre Custody replay"),
    );
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        INITIAL_HOARD_ATOMS,
        "the founding's principal must still be exactly where the founding left it"
    );
    assert!(
        !result
            .logs
            .iter()
            .any(|log| log == &format!("Program {CUSTODY_PROGRAM_ID} success")),
        "a substituted namespace must not reach a successful Custody transfer: {:#?}",
        result.logs
    );
}

/// The coordinate every payout route assumed before this campaign.
///
/// `FoundingV5` wrote `request.market()` into `custody_context` while the same
/// instruction had already authenticated the founding's real namespace, so an
/// aggregate saying "Market address" is exactly the state the tree used to
/// produce. It must now refuse rather than redeem against a Hoard that is not
/// the founded one.
#[tokio::test]
async fn the_market_address_is_not_a_custody_namespace() {
    a_substituted_custody_namespace_refuses(
        |fixture| fixture.market.to_bytes(),
        "the Market address",
    )
    .await;
}

/// A different founding's namespace, offered to this Market.
///
/// `GenericFoundingRequestV1::context` is caller-owned, so a second founding is
/// a second 32 bytes and nothing more. The Vault seeds pin `market` and
/// `release_set` either side of the context, so this cannot reach another
/// Market's principal -- but it must not be allowed to name a compartment of
/// THIS Market that the founding never funded either.
#[tokio::test]
async fn another_foundings_namespace_is_not_this_markets() {
    a_substituted_custody_namespace_refuses(
        |_| hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, &[0x70; 32]]).to_bytes(),
        "a second founding's context digest",
    )
    .await;
}

/// The founding's own action context, unhashed.
///
/// The namespace is the DIGEST, not the context: `open_hoard` derives the Vault
/// under `SHA-256(domain || context)` and the raw context names the Settlement
/// funding-source compartment instead. Off by one hash is off by a whole
/// compartment, and this is the case that says so.
#[tokio::test]
async fn the_unhashed_founding_context_is_not_the_namespace() {
    a_substituted_custody_namespace_refuses(
        |_| FOUNDING_ACTION_CONTEXT_V1,
        "the raw founding action context",
    )
    .await;
}

// ---------------------------------------------------------------------------
// The redemption's step two: a PLAIN WALLET-HELD Position gets paid.
//
// Everything above redeems through the Rational representation: the debited
// Position is owned by a Claims capability PDA and the wallet's entitlement is
// proved by burning shards. The fixture has ALWAYS also carried
// `fixture.actor_position` -- an LBV2 Position at `(aggregate, actor.pubkey())`
// holding `ACTOR_CLAIMS` -- and until now nothing in the tree could pay it.
// `claims/terminal_settlement_v3::process` computes exactly the right number and
// moves exactly the right atoms; its admission took `Core | Trading` callers, so
// the only party who could never reach it was the one who owns the claims.
//
// The widened admission is role `Claims`: this program executing top-level with
// no caller program, where coordinate 0 carries the OWNER'S OWN SIGNATURE. A
// program-derived address has no private key, so that signature is itself the
// proof the Position is wallet-held.
// ---------------------------------------------------------------------------

/// Everything a wallet payout may be bent by, for the hostiles.
///
/// `Eq` because the honest payout is the one that bends NOTHING, and
/// [`wallet_payout_instruction`] asks exactly that question before deciding
/// whether the operator may author the wire.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct WalletPayoutOverrides {
    /// Execution role on the wire.
    caller_role: Option<CallerRole>,
    /// `request.owner` -- the Position owner the evaluator will join.
    owner: Option<[u8; 32]>,
    /// The account at coordinate 0, when it must differ from `request.owner`.
    authority: Option<Pubkey>,
    /// The Position account, when it must differ from the canonical one.
    position_account: Option<Pubkey>,
    /// `request.position`.
    position: Option<[u8; 32]>,
    /// The claim coordinate debited.
    claim_index: Option<u32>,
    /// The claim atoms debited.
    quantity: Option<u64>,
    /// The Core terminal receipt this request claims to settle.
    terminal_record_digest: Option<[u8; 32]>,
    /// Certificate account presented at the authenticated Resolution seam.
    terminal_certificate_account: Option<Pubkey>,
    /// ProgramData presented for the activated Resolution role.
    resolution_programdata_account: Option<Pubkey>,
    /// The optimistic Custody replay cursor.
    expected_custody_revision: Option<u64>,
    /// Coordinate 14/15, when they must not be this program.
    caller_program: Option<(Pubkey, Pubkey)>,
    /// Make the owner the transaction's FEE PAYER rather than a second signer.
    owner_pays_the_fee: bool,
    /// Present the authority coordinate without its signature.
    authority_withholds_its_signature: bool,
    /// The optimistic aggregate revision.
    expected_market_revision: Option<u64>,
    /// The optimistic Position revision.
    expected_position_revision: Option<u64>,
    /// This hostile corrupts CHAIN STATE the operator reads, so the operator
    /// refuses to build the frame at all and the campaign must build it itself.
    ///
    /// It exists so the fallback is never silent. Every other honest payout
    /// gets its wire from `build_wallet_terminal_payout_v3`, and an operator
    /// refusal anywhere else is a hard failure rather than a quiet return to
    /// the hand-built frame. The one site that sets it
    /// (`a_cross_market_position_is_not_payable_here`) also asserts WHICH
    /// refusal the operator gives, so "the builder catches this offline" is a
    /// measurement and not a claim.
    prestate_the_operator_refuses: bool,
}

/// The exact 640-byte request for one wallet payout out of this fixture.
///
/// Nothing here is a restatement: every identity is read off the fixture's own
/// records, which is what the browser will do off the chain.
fn wallet_payout_request(
    fixture: &Fixture,
    overrides: WalletPayoutOverrides,
) -> TerminalSettlementRequestV3 {
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
        caller_role: overrides.caller_role.unwrap_or(CallerRole::Claims),
        release_set: fixture.release_set,
        market: fixture.market.to_bytes(),
        realm: fixture.realm_id,
        parent_context: fixture.parent_context,
        product_record_digest: fixture.product_digest,
        exposure_id: fixture.graph_id,
        exposure_digest: fixture.graph_digest,
        terminal_record_digest: overrides.terminal_record_digest.unwrap_or(
            fixture
                .terminal_record_digest
                .expect("a resolved Market carries a terminal receipt"),
        ),
        owner: overrides.owner.unwrap_or(fixture.actor.pubkey().to_bytes()),
        position: overrides
            .position
            .unwrap_or(fixture.actor_position.to_bytes()),
        recipient_owner: fixture.actor.pubkey().to_bytes(),
        recipient_token_account: terminal.recipient.to_bytes(),
        claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
        custody_program: CUSTODY_PROGRAM_ID.to_bytes(),
        collateral_mint: terminal.collateral_mint.to_bytes(),
        token_program: TOKEN_PROGRAM_ID.to_bytes(),
        semantic_basis_id: fixture.semantic_basis_id,
        linked_basis_record_digest: fixture.linked_basis_digest,
        generation: GENERATION,
        expected_market_revision: overrides.expected_market_revision.unwrap_or(0),
        expected_position_revision: overrides.expected_position_revision.unwrap_or(0),
        expected_custody_revision: overrides
            .expected_custody_revision
            .unwrap_or(CUSTODY_EXPECTED_REVISION),
        quantity: overrides.quantity.unwrap_or(
            ACTOR_CLAIMS
                .get(usize::try_from(fixture.terminal_winner).expect("winner index"))
                .copied()
                .expect("actor claims at the terminal coordinate"),
        ),
        claim_index: overrides.claim_index.unwrap_or(fixture.terminal_winner),
        transfer_index: 0,
    })
    .expect("canonical wallet payout request")
}

/// The Product-to-Claims projection one wallet payout is evaluated under.
///
/// One author, because the campaign's own planner and the operator input built
/// beside it both need it and a second construction of a projection is a second
/// opinion about what the Market sold.
fn wallet_payout_admission(
    fixture: &Fixture,
    request: TerminalSettlementRequestV3,
) -> ProductClaimsTerminalAdmissionV3 {
    let input = request.input();
    ProductClaimsTerminalAdmissionV3::new(
        input.exposure_id,
        input.exposure_digest,
        [0x61; 32],
        fixture.result_domain_digest,
        [0x62; 32],
        [0x63; 32],
        input.semantic_basis_id,
        input.linked_basis_record_digest,
        input.market,
        input.release_set,
        [0x68; 32],
        OUTCOME_COUNT,
        fixture.basis_profile.payout_scale(),
    )
    .expect("terminal admission projection")
}

/// The same wallet payout, built by the OPERATOR a wallet actually calls.
///
/// `crates/dclutch-operator/src/wallet_terminal_payout_v3.rs` is what the CLI,
/// the browser and any redemption builder run. Until 2026-09-02 no real-ELF
/// campaign called it: this file hand-built the same thirty-six-account frame
/// and the two were never compared, which is a second author for a wire with
/// one owner. [`wallet_payout_instruction`] now asks this one to build every
/// honest payout and checks the hand-built frame against it coordinate by
/// coordinate, so a divergence is a failure at every honest call site rather
/// than a discovery at a refused redemption.
///
/// The observation is labelled `Finalized` because the operator refuses
/// anything else, and saying so is the honest form: ProgramTest HAS no
/// finalized commitment (`tools/gauntlet/TIERS.md`), so this label asserts that
/// the fixture's accounts are a single consistent prestate, not that a cluster
/// finalized them.
fn wallet_payout_operator_report(
    fixture: &Fixture,
    overrides: WalletPayoutOverrides,
    prestate: &WalletPayoutPrestate,
) -> Result<WalletTerminalPayoutReportV3, WalletTerminalPayoutErrorV3> {
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let request = wallet_payout_request(fixture, overrides);
    let admission = wallet_payout_admission(fixture, request);
    let input = request.input();
    build_wallet_terminal_payout_v3(WalletTerminalPayoutInputV3 {
        observation: Observation {
            slot: 1,
            unix_timestamp: 0,
            finality: Finality::Finalized,
        },
        route: WalletTerminalPayoutRouteV3 {
            aggregate: fixture.aggregate,
            linked_basis_raw: fixture.linked_basis_record,
            linked_basis_staging: fixture.linked_basis_staging,
            product_raw: fixture.product_record,
            product_staging: fixture.product_staging,
            result_domain_raw: fixture.result_domain_record,
            result_domain_staging: fixture.result_domain_staging,
            portfolio_raw: fixture.portfolio_record,
            portfolio_staging: fixture.portfolio_staging,
            market: fixture.market,
            activation_cache: fixture.activation_cache,
            registry_program: REGISTRY_PROGRAM_ID,
            claims_program: CLAIMS_PROGRAM_ID,
            claims_programdata: fixture.claims_programdata,
            core_program: CORE_PROGRAM_ID,
            core_programdata: fixture.core_programdata,
            resolution_program: RESOLUTION_PROGRAM_ID,
            resolution_programdata: fixture.resolution_programdata,
            position: fixture.actor_position,
            exposure_raw: fixture.graph_raw,
            exposure_staging: fixture.graph_staging,
            custody_program: CUSTODY_PROGRAM_ID,
            terminal_certificate: terminal.certificate,
            realm_raw: terminal.realm_raw,
            realm_staging: terminal.realm_staging,
            custody_replay: terminal.custody_replay,
            collateral_mint: terminal.collateral_mint,
            hoard: terminal.hoard,
            recipient: terminal.recipient,
            custody_authority: terminal.custody_authority,
            token_program: TOKEN_PROGRAM_ID,
        },
        parent_context: fixture.parent_context,
        terminal_record_digest: input.terminal_record_digest,
        recipient_owner: input.recipient_owner,
        transfer_index: input.transfer_index,
        admission,
        product_basis_bytes: &fixture.linked_basis_bytes,
        composition_exposure_bytes: &fixture.graph_bytes,
        composition_exposure_admission: RecordAdmissionV3 {
            selected_id: input.exposure_id,
            finalized_id: input.exposure_id,
            recomputed_digest: fixture.graph_digest,
            finalized_digest: input.exposure_digest,
            record_authenticated: true,
        },
        product_record_digest: input.product_record_digest,
        aggregate_bytes: &prestate.aggregate,
        position_bytes: &prestate.position,
        custody_replay_bytes: &prestate.custody_replay,
        hoard_token_bytes: &prestate.hoard_token,
        recipient_token_bytes: &prestate.recipient_token,
        terminal: fixture.terminal_scenario,
        owner: input.owner,
        claim_index: input.claim_index,
        quantity: input.quantity,
        expected_generation: input.generation,
        expected_market_revision: input.expected_market_revision,
        expected_position_revision: input.expected_position_revision,
    })
}

/// Rebuild, host-side, the packet and payout the chain will derive.
///
/// The Custody caller PDA depends on the Custody request digest, which depends
/// on the candidate digest, which depends on the packet digest and the payout.
/// So a builder cannot address this frame without evaluating the Product -- and
/// this function is the campaign's proof that the arithmetic is reproducible
/// from public state alone. It is also what a browser builder has to do.
fn wallet_payout_plan(
    fixture: &Fixture,
    request_bytes: &[u8],
    prestate: &WalletPayoutPrestate,
) -> Option<(u64, [u8; 32])> {
    let request = TerminalSettlementRequestV3::decode(request_bytes).expect("canonical request");
    let input = request.input();
    let admission = wallet_payout_admission(fixture, request);
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral delta");
    let mut payouts = vec![0_u64; K];
    let mut translation_scratch = vec![0_u64; K];
    let mut claims_payouts = vec![0_u64; K];
    let mut aggregate_deltas = vec![neutral; K];
    let mut packet = vec![0_u8; plan_bytes(OUTCOME_COUNT, 1, 1).expect("wallet payout packet")];
    let payout = encode_product_claims_terminal_signed_delta_v3(
        ProductClaimsTerminalInputV3 {
            product_basis_bytes: &fixture.linked_basis_bytes,
            admission,
            composition_exposure_bytes: &fixture.graph_bytes,
            composition_exposure_admission: RecordAdmissionV3 {
                selected_id: input.exposure_id,
                finalized_id: input.exposure_id,
                recomputed_digest: fixture.graph_digest,
                finalized_digest: input.exposure_digest,
                record_authenticated: true,
            },
            product_record_digest: input.product_record_digest,
            market_account: fixture.aggregate.to_bytes(),
            market_bytes: &prestate.aggregate,
            position_bytes: &prestate.position,
            owner: input.owner,
            request_id: hash(request_bytes).to_bytes(),
            caller_role: input.caller_role,
            terminal: fixture.terminal_scenario,
            claim_index: input.claim_index,
            quantity: input.quantity,
            expected_generation: input.generation,
            expected_market_revision: input.expected_market_revision,
            expected_position_revision: input.expected_position_revision,
            hoard_before: prestate.hoard,
        },
        &mut payouts,
        &mut translation_scratch,
        &mut claims_payouts,
        &mut aggregate_deltas,
        &mut packet,
    )
    .ok()?;
    Some((payout, hash(&packet).to_bytes()))
}

/// The live economic prestate one wallet payout is built against.
///
/// Read off the chain rather than restated from fixture constants, because the
/// Custody caller PDA depends on the payout, which depends on the Position's and
/// the aggregate's ACTUAL contents. A second redemption after a partial one has
/// a different prestate and a different address, and a builder that assumed the
/// genesis numbers would address the wrong account. So does a browser.
struct WalletPayoutPrestate {
    aggregate: Vec<u8>,
    position: Vec<u8>,
    hoard: u64,
    /// Exact Claims-role Custody replay bytes. The campaign's own builder takes
    /// the cursor as an override constant; the operator DECODES it and derives
    /// `expected_custody_revision` from what the chain actually holds, which is
    /// one of the three places the two authors could disagree.
    custody_replay: Vec<u8>,
    /// Exact Hoard and recipient token-account bytes, for the same reason.
    hoard_token: Vec<u8>,
    recipient_token: Vec<u8>,
}

async fn wallet_payout_prestate(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    position: Pubkey,
) -> WalletPayoutPrestate {
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let hoard = observed(context, terminal.hoard).await;
    WalletPayoutPrestate {
        aggregate: observed(context, fixture.aggregate).await.data,
        position: observed(context, position).await.data,
        hoard: token_amount(&hoard),
        custody_replay: observed(context, terminal.custody_replay).await.data,
        hoard_token: hoard.data.clone(),
        recipient_token: observed(context, terminal.recipient).await.data,
    }
}

/// The Custody caller-authority PDA this route will sign for.
///
/// When the plan does not evaluate at all -- a request whose owner does not own
/// the Position, say -- there is no address to derive, and none is needed: such a
/// request is refused by the evaluator long before Custody is reached. The
/// inert Claims program id stands there so the hostile still presents a
/// well-formed frame and fails for the reason it is testing.
fn wallet_payout_custody_caller(
    fixture: &Fixture,
    request_bytes: &[u8],
    prestate: &WalletPayoutPrestate,
) -> Pubkey {
    let request = TerminalSettlementRequestV3::decode(request_bytes).expect("canonical request");
    let input = request.input();
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let request_digest = hash(request_bytes).to_bytes();
    let Some((payout, packet_digest)) = wallet_payout_plan(fixture, request_bytes, prestate) else {
        return CLAIMS_PROGRAM_ID;
    };
    let candidate_digest = hashv(&[
        TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        &request_digest,
        &packet_digest,
        &payout.to_le_bytes(),
        &input.exposure_digest,
        &input.terminal_record_digest,
    ])
    .to_bytes();
    let custody = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CustodyCallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: input.release_set,
        market: input.market,
        realm: input.realm,
        context: fixture.custody_context,
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: [0; 32],
            destination_owner: input.recipient_owner,
            order: [0; 32],
            parent_request_digest: request_digest,
            order_nonce: input.expected_position_revision,
            generation: input.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: input.transfer_index,
        },
        source: terminal.hoard.to_bytes(),
        // Read from the request, not from the fixture. The chain does exactly
        // this -- `authenticate_extra_privileges` binds the recipient ACCOUNT
        // to `input.recipient_token_account` -- so hardcoding the fixture's own
        // recipient here made the helper a second author for a field the
        // request already carries. Identical for every wallet payout, and the
        // difference is what a payout to any other destination needs.
        destination: input.recipient_token_account,
        source_vault_context: fixture.custody_context,
        destination_vault_context: [0; 32],
        mint: input.collateral_mint,
        token_program: input.token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: input.expected_custody_revision,
        resulting_revision: input
            .expected_custody_revision
            .checked_add(1)
            .expect("resulting revision"),
        amount: payout,
        rent_lamports: 0,
    };
    if payout == 0 {
        // There is no Custody CPI to authorize. `authenticate_zero_custody_accounts`
        // requires this coordinate to BE the Claims program -- an inert value
        // that cannot be mistaken for an authority -- and a zero-amount
        // `CustodyRequestV1` would not even validate.
        return CLAIMS_PROGRAM_ID;
    }
    let custody_digest = hash(&custody.to_bytes().expect("Custody request")).to_bytes();
    Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(input.release_set).expect("release set"),
            input.market,
            ExecutionRoleV1::Claims,
            fixture.custody_context,
            custody_digest,
        )
        .expect("Claims Custody caller seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0
}

/// The exact 36-account terminal-settlement frame for a wallet payout.
fn wallet_payout_instruction(
    fixture: &Fixture,
    overrides: WalletPayoutOverrides,
    prestate: &WalletPayoutPrestate,
) -> (Instruction, Vec<u8>) {
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let request = wallet_payout_request(fixture, overrides);
    let request_bytes = request.to_bytes().to_vec();
    let authority = overrides.authority.unwrap_or(fixture.actor.pubkey());
    let position = overrides.position_account.unwrap_or(fixture.actor_position);
    let (caller_program, caller_programdata) = overrides
        .caller_program
        .unwrap_or((CLAIMS_PROGRAM_ID, fixture.claims_programdata));
    let accounts = vec![
        // Coordinate 0. Under `Core`/`Trading` this is a release-pinned caller
        // PDA; under `Claims` it is the owner, who can sign because it has a key.
        if overrides.owner_pays_the_fee {
            AccountMeta::new(authority, true)
        } else {
            AccountMeta::new_readonly(authority, !overrides.authority_withholds_its_signature)
        },
        AccountMeta::new(fixture.aggregate, false),
        AccountMeta::new_readonly(fixture.linked_basis_record, false),
        AccountMeta::new_readonly(fixture.linked_basis_staging, false),
        AccountMeta::new_readonly(fixture.product_record, false),
        AccountMeta::new_readonly(fixture.product_staging, false),
        AccountMeta::new_readonly(fixture.result_domain_record, false),
        AccountMeta::new_readonly(fixture.result_domain_staging, false),
        AccountMeta::new_readonly(fixture.portfolio_record, false),
        AccountMeta::new_readonly(fixture.portfolio_staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(fixture.market, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(caller_program, false),
        AccountMeta::new_readonly(caller_programdata, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.claims_programdata, false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(fixture.core_programdata, false),
        AccountMeta::new(position, false),
        AccountMeta::new_readonly(fixture.graph_raw, false),
        AccountMeta::new_readonly(fixture.graph_staging, false),
        AccountMeta::new_readonly(
            wallet_payout_custody_caller(fixture, &request_bytes, prestate),
            false,
        ),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(
            overrides
                .terminal_certificate_account
                .unwrap_or(terminal.certificate),
            false,
        ),
        AccountMeta::new_readonly(RESOLUTION_PROGRAM_ID, false),
        AccountMeta::new_readonly(
            overrides
                .resolution_programdata_account
                .unwrap_or(fixture.resolution_programdata),
            false,
        ),
        AccountMeta::new_readonly(terminal.realm_raw, false),
        AccountMeta::new_readonly(terminal.realm_staging, false),
        AccountMeta::new(terminal.custody_replay, false),
        AccountMeta::new_readonly(terminal.collateral_mint, false),
        AccountMeta::new(terminal.hoard, false),
        AccountMeta::new(terminal.recipient, false),
        AccountMeta::new_readonly(terminal.custody_authority, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ];
    assert_eq!(accounts.len(), TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3);
    let hand_built = Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts,
        data: request_bytes.clone(),
    };
    if overrides.prestate_the_operator_refuses {
        // The operator READS the accounts this hostile corrupted and refuses
        // offline; the campaign still has to put the frame on the wire to prove
        // what the CHAIN does with it. The setting site asserts the refusal.
        assert!(
            wallet_payout_operator_report(fixture, overrides, prestate).is_err(),
            "this override says the operator refuses this prestate, and it did not"
        );
        return (hand_built, request_bytes);
    }
    if overrides != WalletPayoutOverrides::default() {
        // A hostile is a frame the operator would refuse to build -- a
        // substituted certificate, an unsigned authority, a foreign caller
        // program. Building those is what this function is for, and the
        // operator is not a second opinion about them.
        return (hand_built, request_bytes);
    }
    // The honest payout has ONE author, and this is where the campaign stops
    // being a second one. Compared coordinate by coordinate before the
    // operator's instruction is returned, so a divergence names the position it
    // is at instead of surfacing as a refusal thirty-six accounts deep.
    let operator = wallet_payout_operator_report(fixture, overrides, prestate)
        .expect("the operator builds every honest wallet payout in this campaign")
        .instruction;
    assert_eq!(
        operator.program_id, hand_built.program_id,
        "the two authors of the wallet payout disagree about the program"
    );
    assert_eq!(
        operator.data, hand_built.data,
        "the two authors of the wallet payout disagree about the request bytes"
    );
    assert_eq!(
        operator.accounts.len(),
        hand_built.accounts.len(),
        "the two authors of the wallet payout disagree about the frame width"
    );
    for (coordinate, (built, hand)) in operator
        .accounts
        .iter()
        .zip(hand_built.accounts.iter())
        .enumerate()
    {
        assert_eq!(
            (built.pubkey, built.is_signer, built.is_writable),
            (hand.pubkey, hand.is_signer, hand.is_writable),
            "the two authors of the wallet payout disagree at coordinate {coordinate}"
        );
    }
    (operator, request_bytes)
}

/// Submit one wallet payout over a live address-lookup table.
///
/// This route CANNOT ride a legacy packet. Its frame is thirty-six accounts and
/// its request is six hundred and forty bytes; the measured legacy encoding
/// exceeds the 1,232-byte limit. That is a protocol fact about terminal
/// settlement, not a campaign choice, and it is the one asymmetry between the
/// redemption's two steps -- step one (`custody_replay_v1`, 711 bytes) is
/// deliberately legacy so a redeemer can always create the cursor, and step two
/// needs a published table. Any redemption builder, including the browser's,
/// has to publish one.
async fn submit_wallet_payout(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    table: Pubkey,
    addresses: &[Pubkey],
    overrides: WalletPayoutOverrides,
    label: &str,
) -> Submission {
    let position = overrides.position_account.unwrap_or(fixture.actor_position);
    let prestate = wallet_payout_prestate(context, fixture, position).await;
    let (instruction, _) = wallet_payout_instruction(fixture, overrides, &prestate);
    let fee_payer = if overrides.owner_pays_the_fee {
        overrides.authority.unwrap_or(fixture.actor.pubkey())
    } else {
        context.payer.pubkey()
    };
    // Every call must produce a distinct transaction even when a hostile keeps
    // the instruction bytes identical. ProgramTest records failed signatures;
    // reusing the current blockhash can therefore prove only AlreadyProcessed
    // (with no program logs) instead of exercising Claims' refusal again.
    let blockhash = context
        .get_new_latest_blockhash()
        .await
        .expect("a distinct wallet-payout blockhash");
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &fee_payer,
            &[compute_unit_limit_instruction(), instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 wallet payout message"),
    );
    // Sign with exactly the keys the compiled message demands, in its order. A
    // hostile that moves the authority coordinate to a different identity moves
    // the signer set with it, and guessing that set is how a campaign starts
    // failing for the wrong reason.
    let required = usize::from(message.header().num_required_signatures);
    let signers: Vec<&Keypair> = message
        .static_account_keys()
        .get(..required)
        .expect("required signers")
        .iter()
        .map(|key| {
            if key == &context.payer.pubkey() {
                &context.payer
            } else {
                assert_eq!(
                    key,
                    &fixture.actor.pubkey(),
                    "the campaign holds no key for this required signer"
                );
                &fixture.actor
            }
        })
        .collect();
    let transaction =
        VersionedTransaction::try_new(message, &signers).expect("signed v0 wallet payout");
    let signature = transaction
        .signatures
        .first()
        .expect("signed transaction")
        .to_string();
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message.serialize().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("wallet payout processing");
    let accepted = processed.result.is_ok();
    let failure = processed.result.err().map(|error| format!("{error:?}"));
    let (logs, compute_units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(compute_units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    Submission {
        accepted,
        compute_units,
        wire_bytes,
        logs,
    }
}

/// Publish the lookup table one wallet payout needs, and hand back its addresses.
async fn wallet_payout_lookup_table(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    label_prefix: &str,
) -> (Pubkey, Vec<Pubkey>) {
    let prestate = wallet_payout_prestate(context, fixture, fixture.actor_position).await;
    let (instruction, _) =
        wallet_payout_instruction(fixture, WalletPayoutOverrides::default(), &prestate);
    // The legacy shape is measured, not assumed: this is the number that says an
    // ordinary transaction cannot carry this route.
    let legacy = legacy_wire_bytes(context.payer.pubkey(), instruction.clone(), Hash::default());
    assert!(
        legacy > PACKET_LIMIT,
        "if a wallet payout ever fits a legacy packet, say so and drop the table: {legacy} bytes"
    );
    let addresses = lookup_addresses(
        context.payer.pubkey(),
        fixture.actor.pubkey(),
        &[instruction],
    );
    let (table, _) = create_live_lookup_table(context, &addresses, label_prefix).await;
    (table, addresses)
}

/// One coordinate's outstanding claim supply in the LBV2 aggregate.
fn lbv2_market_supply(bytes: &[u8], outcome: u32) -> u64 {
    let index = usize::try_from(outcome).expect("outcome index");
    let offset = LiabilityBasisMarketLayoutV2::SUPPLIES
        .checked_add(
            index
                .checked_mul(LiabilityBasisMarketLayoutV2::SUPPLY_STRIDE)
                .expect("supply offset"),
        )
        .expect("supply offset");
    u64::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(8).expect("supply end"))
            .expect("LBV2 aggregate supply")
            .try_into()
            .expect("LBV2 supply width"),
    )
}

/// The Claims-role Custody replay's cursor.
///
/// Not [`replay_revision`], which reads the REPRESENTATION replay: these are two
/// different accounts with two different widths, and reading one with the
/// other's decoder is exactly the confusion decision 0008 §7 exists about.
fn custody_replay_revision(account: &Account) -> u64 {
    CustodyReplayV1::decode(&account.data)
        .expect("live Claims-role replay")
        .next_revision
}

/// Refuse-and-prove: the named custom code appeared and no value moved.
fn assert_refused_with(result: &Submission, code: u32, label: &str) {
    assert!(!result.accepted, "{label} must refuse: {:#?}", result.logs);
    assert!(
        result.logs.iter().any(|log| log
            == &format!("Program {CLAIMS_PROGRAM_ID} failed: custom program error: {code:#x}")),
        "{label} must refuse with {code:#x}: {:#?}",
        result.logs
    );
}

/// A degree-two ProductBasisV3 pays its exact cumulative-floor partition
/// through the real Claims, Custody, Core, Registry, Resolution and Token-2022
/// ELFs.
///
/// The knot vector is the clamped quadratic basis `[0,0,0,3,3,3]`. At the
/// Resolution-owned coordinate `3/2`, its exact weights are `[1/4,1/2,1/4]`;
/// at scale seven the one named rounding boundary yields `[1,4,2]`. Redeeming
/// the wallet's two middle-coordinate claims must therefore move eight
/// collateral atoms. A categorical fixture would move two,
/// so this is an executable discriminator for the evaluator family.
#[tokio::test]
async fn a_degree_two_curve_pays_the_cumulative_floor_partition_on_real_elfs() {
    let (test, fixture) = fixture_with_basis(
        TerminalV1::Provider,
        ReceiptMintRoles::Both,
        ProductBasisProfileV1::CurvedDegreeTwo,
    );
    let basis = ProductBasisV3::decode(&fixture.linked_basis_bytes)
        .expect("finalized curved ProductBasisV3");
    let mut payouts = [0_u64; K];
    basis
        .evaluate_rational(
            CURVED_RESULT_NUMERATOR,
            CURVED_RESULT_DENOMINATOR,
            &mut payouts,
        )
        .expect("curved terminal payout");
    assert_eq!(
        payouts,
        ProductBasisProfileV1::CurvedDegreeTwo.expected_curve_payouts()
    );
    assert_eq!(fixture.initial_hoard_atoms, 59);

    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: curved wallet payout",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;
    let quantity = ACTOR_CLAIMS[WINNERS];
    let paid = quantity
        .checked_mul(payouts[WINNERS])
        .expect("curved payout fits");
    assert_eq!(paid, 8);

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides::default(),
        "claims rational-representation-v2: degree-two cumulative-floor payout",
    )
    .await;
    if !result.accepted {
        eprintln!("curved payout refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(result.accepted, "the curved payout must commit");
    assert!(result.wire_bytes <= PACKET_LIMIT);

    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        fixture.initial_hoard_atoms - paid
    );
    assert_eq!(
        token_amount(after.recipient.as_ref().expect("recipient")),
        INITIAL_RECIPIENT_ATOMS + paid
    );
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard"))
            + token_amount(after.recipient.as_ref().expect("recipient")),
        fixture.initial_hoard_atoms + INITIAL_RECIPIENT_ATOMS,
        "collateral is conserved independently of the rounded partition"
    );
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, WINNER),
        0
    );
    assert_eq!(
        lbv2_market_supply(&after.aggregate.data, WINNER),
        aggregate_claims()[WINNERS] - quantity
    );
    for index in 0..K {
        if index == WINNERS {
            continue;
        }
        let outcome = u32::try_from(index).expect("outcome index");
        assert_eq!(
            lbv2_market_supply(&after.aggregate.data, outcome),
            lbv2_market_supply(&before.aggregate.data, outcome),
            "the payout debits only the selected native claim"
        );
    }
    for index in 0..K {
        assert_account_content_eq(
            after.positions.get(index).expect("custody Position"),
            before.positions.get(index).expect("pre custody Position"),
        );
        assert_account_content_eq(
            after.shard_mints.get(index).expect("shard Mint"),
            before.shard_mints.get(index).expect("pre shard Mint"),
        );
    }
}

/// STEP TWO OF THE REDEMPTION: a plain wallet's Position gets paid.
///
/// The actor is an ordinary keypair. Its Position is the canonical LBV2 PDA at
/// `(aggregate, actor)`, it holds `ACTOR_CLAIMS` and nothing has ever been able
/// to pay it: `terminal_settlement_v3` computed exactly the right number and
/// admitted `Core | Trading` callers only, so the one party who could not reach
/// it was the one who owns the claims.
///
/// Here the actor signs for itself under execution role `Claims` and the whole
/// chain closes: signer == `request.owner` == the Position header's owner == the
/// canonical Position PDA's second seed.
#[tokio::test]
async fn a_wallet_held_position_is_paid_from_the_resolved_markets_hoard() {
    // The Trading role is the real Trading program here. See
    // `fixture_with_real_trading_role`: this is the campaign's one payout on a
    // release set shaped like a cohort's, and it is the payout the browser's
    // live redemption drives.
    let (test, fixture) = fixture_with_real_trading_role(TerminalV1::Provider);
    let mut context = test.start_with_context().await;

    // The bank's own activation says which program the Trading role is, and it
    // is asserted rather than assumed: a fixture swap that quietly kept the
    // test caller in that slot would prove exactly nothing, and this campaign's
    // other forty-six cases still run on the caller-as-Trading release set.
    // Read from the ACCOUNT the chain will authenticate against, not from the
    // constructor that wrote it.
    let activated = ActivatedExecutionReleaseSetV1::decode(
        &observed(&mut context, fixture.activation_cache).await.data,
    )
    .expect("the Registry-owned activation cache");
    assert_eq!(
        activated
            .role(ExecutionRoleV1::Trading)
            .release()
            .program()
            .to_bytes(),
        TRADING_PROGRAM_ID.to_bytes(),
        "this payout must execute on a bank whose Trading role is the real Trading program",
    );
    assert_ne!(
        fixture.release_set,
        activation_cache(&artifacts()).0,
        "the real Trading role must move the release-set id off the test caller's",
    );

    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: wallet payout",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;
    assert_eq!(
        lbv2_position_quantity(&before.actor_position.data, WINNER),
        ACTOR_CLAIMS[WINNERS],
        "the wallet's own claims at the winning coordinate"
    );

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides::default(),
        "claims rational-representation-v2: a wallet-held Position is paid",
    )
    .await;
    if !result.accepted {
        eprintln!("wallet payout refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(result.accepted, "the wallet payout must commit");
    assert!(
        result.wire_bytes <= PACKET_LIMIT,
        "the v0 shape must fit a packet once the table carries the frame: {} bytes",
        result.wire_bytes
    );

    let after = snapshot(&mut context, &fixture).await;
    let paid = ACTOR_CLAIMS[WINNERS];
    // Collateral: the Hoard pays exactly `quantity * payout_per_claim`, which for
    // a CategoricalQ1 basis at payout scale one is `quantity`.
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        INITIAL_HOARD_ATOMS - paid
    );
    assert_eq!(
        token_amount(after.recipient.as_ref().expect("recipient")),
        INITIAL_RECIPIENT_ATOMS + paid
    );
    // Claims: the wallet's winning coordinate is burned, and the aggregate's
    // outstanding supply falls by the same atoms. Nothing else moves.
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, WINNER),
        0
    );
    assert_eq!(
        lbv2_market_supply(&after.aggregate.data, WINNER),
        aggregate_claims()[WINNERS] - paid
    );
    for index in 0..K {
        if index == WINNERS {
            continue;
        }
        let outcome = u32::try_from(index).expect("outcome index");
        assert_eq!(
            lbv2_market_supply(&after.aggregate.data, outcome),
            lbv2_market_supply(&before.aggregate.data, outcome),
            "a terminal payout touches exactly the coordinate it debits"
        );
    }
    assert_eq!(lbv2_revision(&after.aggregate.data), 1);
    assert_eq!(lbv2_revision(&after.actor_position.data), 1);
    // The Claims-role cursor advanced exactly once, from the value
    // `InitializeReplay` wrote.
    assert_eq!(
        custody_replay_revision(after.custody_replay.as_ref().expect("Claims-role replay")),
        CUSTODY_EXPECTED_REVISION + 1
    );
    // The representation layer is untouched: this redemption went nowhere near
    // the shard Mints or the Claims capability Positions.
    for index in 0..K {
        assert_account_content_eq(
            after.positions.get(index).expect("custody Position"),
            before.positions.get(index).expect("pre custody Position"),
        );
        assert_account_content_eq(
            after.shard_mints.get(index).expect("shard Mint"),
            before.shard_mints.get(index).expect("pre shard Mint"),
        );
    }
}

/// THE CONVENTIONAL DESTINATION, ON REAL ELFS: Claims admits it, Custody does not.
///
/// Under Token-2022 the Associated Token Account program ALWAYS writes the
/// `ImmutableOwner` extension, so every ordinary wallet's ATA is 165 base bytes
/// plus an account-type byte plus one empty TLV entry -- exactly 170. Cohort-13
/// measured that on chain: the founder's ATA was created, read back at 170
/// bytes, and the payout refused it, so the redemption had to be paid into a
/// 165-byte account created by hand -- a thing no stranger with a browser can
/// do. `TokenAccount::parse_base_or_immutable_owner` is the admission, and
/// `rational_terminal_v3::token_amount` and the host-side builder both use it.
///
/// THIS TEST EXISTS TO SAY EXACTLY HOW FAR THAT GOT, on the real Claims,
/// Custody and Token-2022 ELFs, because "the parse admits it" and "the payout
/// lands" are different claims and only one of them is true today.
///
/// It gets past Claims and stops inside CUSTODY, which authenticates both
/// transfer participants through `ExactTransferProfileV1::check_transfer_account`
/// -- and that profile is not a function anyone may quietly widen. Its
/// `ExtensionStoragePolicy::ExactBaseWidthsOnly` byte is inside the
/// `CollateralAdapterReleaseV1` PREIMAGE, the realm record on chain stores the
/// SHA-256 of that preimage as `collateral_adapter_release_id`, and Custody
/// SELECTS the profile by matching that stored id against the tree's known
/// releases. Redefining the existing release would strand every market founded
/// under it, cohort-13's included, the moment the tree and the chain disagreed
/// about what one id means.
///
/// So the wall is real, it is not in Claims, and its repair is a THIRD adapter
/// release -- a new `ExtensionStoragePolicy` and a new profile variant, added
/// beside the two that exist rather than replacing either -- which a realm
/// founded by a later cohort selects. `docs/design/` owes that its own note;
/// this test owes the measurement, which is that the refusal is Custody's
/// `TokenState`, on real bytes, at the destination and nowhere earlier.
///
/// The control is `a_wallet_held_position_is_paid_from_the_resolved_markets_hoard`:
/// the same payout, the same frame, a 165-byte destination, and it commits.
#[tokio::test]
async fn a_conventional_170_byte_associated_token_account_is_refused_by_custody_alone() {
    let (test, fixture) = fixture_with_real_trading_role(TerminalV1::Provider);
    let mut context = test.start_with_context().await;
    let terminal = fixture.terminal_accounts.expect("terminal fixture");

    // The ATA program's own output shape, planted over the fixture's base
    // account: the same mint, owner and balance, five bytes longer.
    let base = observed(&mut context, terminal.recipient).await;
    let mut suffixed = base.data.clone();
    assert_eq!(
        suffixed.len(),
        ACCOUNT_BYTES,
        "the fixture plants a base account"
    );
    suffixed.extend_from_slice(&IMMUTABLE_OWNER_ACCOUNT_SUFFIX);
    assert_eq!(suffixed.len(), IMMUTABLE_OWNER_ACCOUNT_BYTES);
    context.set_account(
        &terminal.recipient,
        &AccountSharedData::from(Account {
            lamports: Rent::default().minimum_balance(suffixed.len()).max(1),
            data: suffixed,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
    let planted = observed(&mut context, terminal.recipient).await;
    assert_eq!(planted.data.len(), IMMUTABLE_OWNER_ACCOUNT_BYTES);
    assert_eq!(
        TokenAccount::parse(&planted.data),
        Err(dclutch_token_svm::Error::InvalidLength),
        "this is exactly the account the base parser refuses",
    );
    assert!(
        TokenAccount::parse_base_or_immutable_owner(&planted.data).is_ok(),
        "and exactly the account the admitting parser takes",
    );

    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: ATA-shaped wallet payout",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    // The host-side builder RUNS. Before this lane it refused offline as
    // `WalletTerminalPayoutErrorV3::Custody`, so no transaction existed and the
    // chain was never asked. Reaching a submitted packet at all is what the
    // operator's own admission bought.
    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides::default(),
        "claims rational-representation-v2: a 170-byte associated token account reaches Custody",
    )
    .await;
    assert!(
        !result.accepted,
        "Custody's released exact-base-width profile must still refuse the suffix",
    );
    // Named from the registry, not typed: taking a dependency on another
    // program's crate to read one discriminant is the wrong edge, and
    // `CustodySbfError::TokenState` is offset 6 of Custody's band.
    let token_state = dclutch_refusal_registry::CUSTODY_REFUSAL_BASE + 6;
    assert!(
        result.logs.iter().any(|log| log
            == &format!(
                "Program {CUSTODY_PROGRAM_ID} failed: custom program error: {token_state:#x}"
            )),
        "the refusal is Custody's TokenState at the destination: {:#?}",
        result.logs,
    );
    // AND NOTHING MOVED. A wall that half-executed would be worse than one that
    // refuses, so the whole prestate is asserted unchanged.
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        INITIAL_HOARD_ATOMS,
    );
    assert_eq!(
        TokenAccount::parse_base_or_immutable_owner(
            &after.recipient.as_ref().expect("recipient").data
        )
        .expect("the ATA still decodes")
        .amount,
        INITIAL_RECIPIENT_ATOMS,
    );
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, WINNER),
        lbv2_position_quantity(&before.actor_position.data, WINNER),
    );
    assert_eq!(
        custody_replay_revision(after.custody_replay.as_ref().expect("Claims-role replay")),
        CUSTODY_EXPECTED_REVISION,
    );
    eprintln!(
        "ATA-shaped wallet payout: recipient={} bytes refusal={token_state:#x} CU={} wire={}",
        after.recipient.as_ref().expect("recipient").data.len(),
        result.compute_units,
        result.wire_bytes,
    );
}

/// THE SAME 170-BYTE DESTINATION, ON A MARKET FOUNDED UNDER THE THIRD RELEASE:
/// it commits, and the collateral goes to the account a browser derives.
///
/// This is the other half of
/// `a_conventional_170_byte_associated_token_account_is_refused_by_custody_alone`,
/// and the two are a matched pair: same real Claims, Custody, Core, Registry,
/// Resolution and Token-2022 ELFs, same frame, same host builder, same planted
/// account bytes, and ONE difference -- the 32 bytes the Realm record stores as
/// `collateral_adapter_release_id`.
///
/// WHY THE REALM AND NOT THE CODE. Custody reaches
/// `ExactTransferProfileV1::check_transfer_account` through
/// `collateral_profile`, which walks `PRODUCTION_ADAPTER_RELEASES` and returns
/// the profile of whichever entry hashes to the realm's stored id. The
/// `ExtensionStoragePolicy` byte is INSIDE that preimage. So a market's ability
/// to pay an ordinary wallet is fixed at founding, in a released identity, and
/// cannot be granted afterwards by any commit -- which is why cohort-13's
/// auxiliary 165-byte account was not a workaround for a defect a later tree
/// repairs. It was the only destination that cohort could ever pay.
///
/// Cohort-14 founds under `430369ce...` and this test is the measurement that
/// says what that buys, on real bytes rather than in a design note.
#[tokio::test]
async fn a_conventional_170_byte_associated_token_account_commits_under_the_third_adapter_release()
{
    let (test, fixture) = fixture_on_the_immutable_owner_release(TerminalV1::Provider);
    let mut context = test.start_with_context().await;
    let terminal = fixture.terminal_accounts.expect("terminal fixture");

    // The SAME planting as the refusal test, byte for byte: the ATA program's
    // own output shape over the fixture's base account.
    let base = observed(&mut context, terminal.recipient).await;
    let mut suffixed = base.data.clone();
    assert_eq!(
        suffixed.len(),
        ACCOUNT_BYTES,
        "the fixture plants a base account"
    );
    suffixed.extend_from_slice(&IMMUTABLE_OWNER_ACCOUNT_SUFFIX);
    assert_eq!(suffixed.len(), IMMUTABLE_OWNER_ACCOUNT_BYTES);
    context.set_account(
        &terminal.recipient,
        &AccountSharedData::from(Account {
            lamports: Rent::default().minimum_balance(suffixed.len()).max(1),
            data: suffixed,
            owner: TOKEN_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
    let planted = observed(&mut context, terminal.recipient).await;
    assert_eq!(planted.data.len(), IMMUTABLE_OWNER_ACCOUNT_BYTES);
    assert_eq!(
        TokenAccount::parse(&planted.data),
        Err(dclutch_token_svm::Error::InvalidLength),
        "the base parser still refuses it; nothing was relaxed",
    );

    // THE REALM IS THE ONLY DIFFERENCE, and it is read back off the account the
    // programs will authenticate rather than taken from the fixture builder.
    let realm = RealmV1::decode(&observed(&mut context, terminal.realm_raw).await.data)
        .expect("the finalized Realm record");
    let founded: [u8; 32] =
        hash(&CollateralAdapterReleaseV1::token_2022_immutable_owner_exact_transfer().to_bytes())
            .to_bytes();
    let cohort_13: [u8; 32] =
        hash(&CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer().to_bytes())
            .to_bytes();
    assert_eq!(realm.collateral_adapter_release_id(), &founded);
    assert_ne!(
        realm.collateral_adapter_release_id(),
        &cohort_13,
        "if these were equal this test would be the refusal test with a different name",
    );

    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: ATA-shaped wallet payout on the third release",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides::default(),
        "claims rational-representation-v2: a 170-byte associated token account is paid",
    )
    .await;
    if !result.accepted {
        eprintln!("ATA payout refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(
        result.accepted,
        "the conventional destination must be payable under the release that admits it",
    );
    assert!(
        result.wire_bytes <= PACKET_LIMIT,
        "the ATA-shaped payout must still fit a packet: {} bytes",
        result.wire_bytes,
    );

    // THE COLLATERAL MOVED, and it moved into the 170-byte account.
    let after = snapshot(&mut context, &fixture).await;
    let paid = ACTOR_CLAIMS[WINNERS];
    let recipient_after = after.recipient.as_ref().expect("recipient");
    assert_eq!(
        recipient_after.data.len(),
        IMMUTABLE_OWNER_ACCOUNT_BYTES,
        "the transfer did not truncate the extension storage it was paid into",
    );
    assert_eq!(
        &recipient_after.data[ACCOUNT_BYTES..],
        IMMUTABLE_OWNER_ACCOUNT_SUFFIX,
        "and the suffix survived byte for byte",
    );
    assert_eq!(
        TokenAccount::parse_base_or_immutable_owner(&recipient_after.data)
            .expect("the ATA decodes after the payout")
            .amount,
        INITIAL_RECIPIENT_ATOMS + paid,
    );
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        INITIAL_HOARD_ATOMS - paid,
    );
    // Claims did its half too: the winning coordinate is burned and the
    // aggregate falls by the same atoms, exactly as on a 165-byte destination.
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, WINNER),
        0
    );
    assert_eq!(
        lbv2_market_supply(&after.aggregate.data, WINNER),
        aggregate_claims()[WINNERS] - paid,
    );
    assert_eq!(
        lbv2_position_quantity(&before.actor_position.data, WINNER),
        paid,
    );
    assert_eq!(
        custody_replay_revision(after.custody_replay.as_ref().expect("Claims-role replay")),
        CUSTODY_EXPECTED_REVISION + 1,
    );
    eprintln!(
        "ATA-shaped wallet payout COMMITS: recipient={} bytes release={} CU={} wire={}",
        recipient_after.data.len(),
        founded
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        result.compute_units,
        result.wire_bytes,
    );
}

/// A WALLET EXITS A MARKET NOBODY RESOLVED, at the terms disclosed up front.
///
/// This is the far end of the funded failure walk, reached by the one party the
/// walk is for. The relayed campaign proves the first half against the real
/// Resolution and Core ELFs -- a source goes silent, a wallet that is nobody
/// walks the market past its own deadline and is paid a fixed bounty, and Core
/// admits the resulting `ResolutionFailure` certificate so the Market ends
/// Terminal at the Product's pre-disclosed failure region
/// (`crates/dclutch-svm-harness/tests/relayed_mainnet_state.rs` and
/// `resolution_core_v3_lifecycle.rs`'s
/// `a_market_walked_to_failure_ends_terminal_on_its_pre_disclosed_terms`). What
/// had never executed anywhere is the half a person actually cares about:
/// taking the collateral home afterwards. Every terminal settlement in this
/// tree, in every campaign, settled a certificate a PROVIDER stood behind.
///
/// Read the persisted Core phase off chain.
async fn core_phase(context: &mut ProgramTestContext, market: Pubkey) -> CorePhase {
    let account = observed(context, market).await;
    CoreState::decode(&account.data)
        .expect("Core state decodes")
        .phase
}

/// Move a resolved Market into `Retiring` the way anybody on the network can.
///
/// A fresh keypair that is not the holder, not the founder and holds no role
/// pays the fee. It signs nothing INSIDE the instruction, because
/// `begin_retiring` refuses every signer among its five accounts
/// (`programs/dclutch-core-sbf/src/begin_retiring.rs:57`) -- which is exactly
/// what makes the transition available to a stranger and what makes this the
/// cheapest attack in the tree.
async fn a_stranger_begins_retiring(context: &mut ProgramTestContext, fixture: &Fixture) {
    let stranger = Keypair::new();
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let rent = Rent::default().minimum_balance(0);
    context
        .banks_client
        .process_transaction(Transaction::new_signed_with_payer(
            &[transfer(
                &context.payer.pubkey(),
                &stranger.pubkey(),
                rent.checked_mul(64).expect("stranger funding"),
            )],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            blockhash,
        ))
        .await
        .expect("the stranger is funded like anyone else");

    let instruction = Instruction {
        program_id: CORE_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.market, false),
            AccountMeta::new_readonly(fixture.activation_cache, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.core_programdata, false),
        ],
        data: Request::administrative(
            Action::BeginRetiring,
            GENERATION,
            Identity::new(fixture.market.to_bytes()).expect("Market identity"),
        )
        .encode()
        .expect("BeginRetiring request")
        .to_vec(),
    };
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[compute_unit_limit_instruction(), instruction],
        Some(&stranger.pubkey()),
        &[&stranger],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed transaction")
        .to_string();
    // MEASURED LIKE EVERY OTHER ROW. This helper recorded `wire_bytes: None`,
    // which is honest but is not a claim that the frame fits, and it was the
    // whole of what `rational-representation-v2-measured-every-transaction`
    // had to report as unmeasured. Same arithmetic as `submit_legacy_signed`:
    // the shortvec signature count, the signatures, and the serialized message.
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message_data().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("BeginRetiring processing");
    let failure = processed.result.err().map(|error| format!("{error:?}"));
    let (logs, compute_units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label: "claims rational-representation-v2: a stranger begins retiring",
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(compute_units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    assert!(
        failure.is_none(),
        "begin_retiring IS permissionless -- this whole test is about what that \
         costs, and it is vacuous if the transition does not land: {failure:?}\n{}",
        logs.join("\n")
    );
}

/// A STRANGER'S RETIREMENT DOES NOT END A HOLDER'S REDEMPTION.
///
/// `begin_retiring` is permissionless by design and refuses all signers
/// (`programs/dclutch-core-sbf/src/begin_retiring.rs:57`), and the transition's
/// own codec doc says what that permissionlessness is for: "Begin retiring
/// while retaining permissionless redemption"
/// (`crates/dclutch-market-core-codec/src/generated.rs:1030`). Every
/// holder-redemption route nonetheless gated the Core phase on exact equality
/// with `Phase::Terminal`, so an arbitrary actor holding no role and named
/// nowhere in this market could, for one transaction fee, end every holder's
/// redemption right -- and brick the market in the same stroke, because
/// retirement needs zero outstanding supply
/// (`programs/dclutch-claims-sbf/src/market_closure_v1.rs:669-681`) and
/// redemption is the only thing that drives supply toward zero. The collateral
/// became unreachable by anyone, including the people who owned it.
///
/// This test runs that attack against the REAL Core ELF and then makes the
/// holder whole anyway. It cannot pass vacuously: it asserts the transition
/// landed and re-reads Core's persisted phase as `Retiring` before the payout
/// is attempted, so a market that quietly stayed `Terminal` fails here instead
/// of flattering the redemption that follows. The payout assertions are the
/// same ones
/// `a_wallet_held_position_is_paid_from_the_resolved_markets_hoard` makes, to
/// the atom, because the point is that retirement changed nothing a holder is
/// owed.
#[tokio::test]
async fn a_stranger_who_begins_retiring_cannot_end_a_holders_redemption() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: retiring payout",
    )
    .await;

    assert_eq!(
        core_phase(&mut context, fixture.market).await,
        CorePhase::Terminal,
        "the fixture must start resolved, or the attack below is not the attack"
    );
    a_stranger_begins_retiring(&mut context, &fixture).await;
    assert_eq!(
        core_phase(&mut context, fixture.market).await,
        CorePhase::Retiring,
        "the stranger moved the Market, and every assertion after this is about \
         a Market a stranger moved"
    );

    let before = snapshot(&mut context, &fixture).await;
    assert_eq!(
        lbv2_position_quantity(&before.actor_position.data, WINNER),
        ACTOR_CLAIMS[WINNERS],
        "the wallet's own claims at the winning coordinate, still owed"
    );

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides::default(),
        "claims rational-representation-v2: a holder is paid after a stranger began retiring",
    )
    .await;
    if !result.accepted {
        eprintln!("retiring payout refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(
        result.accepted,
        "a stranger must not be able to end this holder's redemption"
    );

    let after = snapshot(&mut context, &fixture).await;
    let paid = ACTOR_CLAIMS[WINNERS];
    // Conservation, asserted over acceptance: the Hoard pays exactly what the
    // Terminal-phase payout pays, and the holder receives exactly that.
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        INITIAL_HOARD_ATOMS - paid
    );
    assert_eq!(
        token_amount(after.recipient.as_ref().expect("recipient")),
        INITIAL_RECIPIENT_ATOMS + paid
    );
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, WINNER),
        0
    );
    assert_eq!(
        lbv2_market_supply(&after.aggregate.data, WINNER),
        aggregate_claims()[WINNERS] - paid
    );
    for index in 0..K {
        if index == WINNERS {
            continue;
        }
        let outcome = u32::try_from(index).expect("outcome index");
        assert_eq!(
            lbv2_market_supply(&after.aggregate.data, outcome),
            lbv2_market_supply(&before.aggregate.data, outcome),
            "a terminal payout touches exactly the coordinate it debits, in \
             Retiring as in Terminal"
        );
    }
    // Redemption during `Retiring` is what drives supply toward the zero that
    // `market_closure_v1` demands, so this is also the step that un-bricks the
    // market the stranger tried to brick.
    assert_eq!(lbv2_revision(&after.aggregate.data), 1);
    assert_eq!(lbv2_revision(&after.actor_position.data), 1);
    assert_eq!(
        custody_replay_revision(after.custody_replay.as_ref().expect("Claims-role replay")),
        CUSTODY_EXPECTED_REVISION + 1
    );
    // The stranger moved the phase and nothing else: the winner and the
    // terminal receipt Core committed at resolution are byte-identical.
    let core = CoreState::decode(&observed(&mut context, fixture.market).await.data)
        .expect("Core state decodes");
    assert_eq!(core.phase, CorePhase::Retiring);
    assert_eq!(core.terminal_winner, fixture.terminal_winner);
    assert!(core.terminal_receipt.is_some());
}

/// So this is not a variant of the payout above. It is the first time a wallet
/// has moved a collateral atom on the authority of a market's own failure, and
/// there is no caller program anywhere between the person and their collateral:
/// the actor signs for itself under execution role `Claims`, and the real
/// Claims ELF invokes the real Custody program, which invokes real Token-2022.
#[tokio::test]
async fn a_wallet_held_position_exits_at_failure_terms_when_nobody_resolved_the_market() {
    let (test, fixture) = fixture_with(TerminalV1::Failure, ReceiptMintRoles::Both);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: failure-terms exit",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;
    let paid = ACTOR_CLAIMS[FAILURE_SELECTORS];
    // Guarded, because an exit that pays nothing is not an exit and this test
    // would still be green if the failure region were ever re-zeroed.
    assert!(
        paid > 0,
        "the wallet must hold claims in the failure region for this to mean anything"
    );
    assert_eq!(
        lbv2_position_quantity(&before.actor_position.data, FAILURE_SELECTOR),
        paid,
        "the wallet's own claims in the pre-disclosed failure region"
    );

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides::default(),
        "claims rational-representation-v2: a wallet exits at failure terms",
    )
    .await;
    if !result.accepted {
        eprintln!(
            "failure-terms exit refusal logs:\n{}",
            result.logs.join("\n")
        );
    }
    assert!(
        result.accepted,
        "a holder must be able to exit on a market's own failure terms"
    );
    assert!(
        result.wire_bytes <= PACKET_LIMIT,
        "the v0 shape must fit a packet once the table carries the frame: {} bytes",
        result.wire_bytes
    );

    let after = snapshot(&mut context, &fixture).await;
    // Collateral is conserved to the atom: what left the Hoard arrived at the
    // holder, and the pair sums to exactly what it opened with.
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        INITIAL_HOARD_ATOMS - paid
    );
    assert_eq!(
        token_amount(after.recipient.as_ref().expect("recipient")),
        INITIAL_RECIPIENT_ATOMS + paid
    );
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard"))
            + token_amount(after.recipient.as_ref().expect("recipient")),
        INITIAL_HOARD_ATOMS + INITIAL_RECIPIENT_ATOMS,
        "no collateral was created or destroyed by the exit"
    );
    // Claims: the failure region is burned out of the wallet and out of the
    // aggregate's outstanding supply, and nothing else moves.
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, FAILURE_SELECTOR),
        0
    );
    assert_eq!(
        lbv2_market_supply(&after.aggregate.data, FAILURE_SELECTOR),
        aggregate_claims()[FAILURE_SELECTORS] - paid
    );
    for index in 0..K {
        if index == FAILURE_SELECTORS {
            continue;
        }
        let outcome = u32::try_from(index).expect("outcome index");
        assert_eq!(
            lbv2_market_supply(&after.aggregate.data, outcome),
            lbv2_market_supply(&before.aggregate.data, outcome),
            "an exit at failure terms touches exactly the coordinate it debits"
        );
    }
    assert_eq!(lbv2_revision(&after.aggregate.data), 1);
    assert_eq!(lbv2_revision(&after.actor_position.data), 1);
    assert_eq!(
        custody_replay_revision(after.custody_replay.as_ref().expect("Claims-role replay")),
        CUSTODY_EXPECTED_REVISION + 1,
        "the collateral moved under a replay-protected order, advanced exactly once"
    );
    // The representation layer is untouched: this exit went nowhere near the
    // shard Mints or the Claims capability Positions.
    for index in 0..K {
        assert_account_content_eq(
            after.positions.get(index).expect("custody Position"),
            before.positions.get(index).expect("pre custody Position"),
        );
        assert_account_content_eq(
            after.shard_mints.get(index).expect("shard Mint"),
            before.shard_mints.get(index).expect("pre shard Mint"),
        );
    }
}

/// Neither kind may occupy the other's coordinate, and nothing moves when it tries.
///
/// `validate_terminal_product` reserves the Product's FINAL coordinate for
/// explicit failure and admits an ordinary success strictly below it. Both
/// halves of that are pinned here, and each hostile is one field away from a
/// case that COMMITS:
///
/// * a provider-backed success selecting the failure region is the exit above
///   with the certificate kind flipped;
/// * failure terms claimed for an ordinary coordinate is
///   `a_wallet_held_position_is_paid_from_the_resolved_markets_hoard` with the
///   certificate kind flipped -- one byte of the whole 640-byte request and
///   36-account frame.
///
/// Those two committing tests are this one's negative controls, which is what
/// makes these refusals evidence rather than decoration: the refusal happens
/// inside the Claims ELF at the certificate seam, after the Custody
/// composition, the Realm and the certificate account have all authenticated.
#[tokio::test]
async fn neither_terminal_kind_may_occupy_the_others_coordinate() {
    for (terminal, label) in [
        (
            TerminalV1::Mismatched {
                winner: FAILURE_SELECTOR,
                kind: ResolutionCertificateKindV2::ResolutionSuccess,
            },
            "a provider success may not occupy the pre-disclosed failure cell",
        ),
        (
            TerminalV1::Mismatched {
                winner: WINNER,
                kind: ResolutionCertificateKindV2::ResolutionFailure,
            },
            "failure terms may not be claimed for an ordinary coordinate",
        ),
    ] {
        let (test, fixture) = fixture_with(terminal, ReceiptMintRoles::Both);
        let mut context = test.start_with_context().await;
        create_claims_custody_replay(&mut context, &fixture).await;
        let (table, addresses) = wallet_payout_lookup_table(&mut context, &fixture, label).await;
        let before = snapshot(&mut context, &fixture).await;

        let result = submit_wallet_payout(
            &mut context,
            &fixture,
            table,
            &addresses,
            WalletPayoutOverrides::default(),
            label,
        )
        .await;
        assert_refused_with(&result, ClaimsSbfError::Identity as u32, label);

        let after = snapshot(&mut context, &fixture).await;
        assert_account_content_eq(
            after.hoard.as_ref().expect("Hoard"),
            before.hoard.as_ref().expect("pre Hoard"),
        );
        assert_account_content_eq(
            after.recipient.as_ref().expect("recipient"),
            before.recipient.as_ref().expect("pre recipient"),
        );
        assert_account_content_eq(&after.actor_position, &before.actor_position);
        assert_account_content_eq(&after.aggregate, &before.aggregate);
        assert_eq!(
            custody_replay_revision(after.custody_replay.as_ref().expect("Claims-role replay")),
            CUSTODY_EXPECTED_REVISION,
            "{label}: a refused settlement fires no Custody CPI"
        );
    }
}

/// The browser's shape: ONE wallet pays the fee and authorizes the redemption.
///
/// The frame spec pins coordinate 0 to a readonly signer, which is right for a
/// caller-authority PDA and impossible for a fee payer -- an account that is both
/// compiles to a single WRITABLE signer entry. Under role `Claims` the pin is
/// relaxed along that one axis, so the account a browser wallet actually
/// produces is admissible. This test is the reason that relaxation is not dead
/// code.
#[tokio::test]
async fn the_position_owner_pays_its_own_fee_and_still_authorizes_the_payout() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: owner-paid wallet payout",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides {
            owner_pays_the_fee: true,
            ..WalletPayoutOverrides::default()
        },
        "claims rational-representation-v2: the Position owner pays its own fee",
    )
    .await;
    if !result.accepted {
        eprintln!("owner-paid refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(
        result.accepted,
        "a single wallet must be able to pay for and authorize its own redemption"
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(
        token_amount(after.hoard.as_ref().expect("Hoard")),
        token_amount(before.hoard.as_ref().expect("pre Hoard")) - ACTOR_CLAIMS[WINNERS]
    );
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, WINNER),
        0
    );
}

/// A losing coordinate pays ZERO and says so honestly.
///
/// The claims are still burned and the aggregate's supply still falls -- a
/// worthless claim is a claim, and retiring it is a real transition -- but no
/// Custody transfer happens at all, the Hoard and the recipient are
/// byte-identical afterwards, and the Claims-role replay cursor does not move.
/// This is `terminal_settlement_v3`'s zero-payout branch, which nothing in the
/// tree had ever executed.
#[tokio::test]
async fn a_losing_coordinate_pays_zero_and_leaves_the_hoard_byte_identical() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: losing wallet coordinate",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;
    let losing = 0_u32;
    assert_ne!(losing, WINNER);
    let held = ACTOR_CLAIMS[0];
    assert!(held > 0, "the wallet must hold a losing claim to burn one");

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides {
            claim_index: Some(losing),
            quantity: Some(held),
            ..WalletPayoutOverrides::default()
        },
        "claims rational-representation-v2: a losing wallet coordinate pays zero",
    )
    .await;
    if !result.accepted {
        eprintln!("losing payout refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(result.accepted, "a zero payout is a commit, not a refusal");
    assert!(
        !result
            .logs
            .iter()
            .any(|log| log == &format!("Program {CUSTODY_PROGRAM_ID} success")),
        "a zero payout must not invoke Custody: {:#?}",
        result.logs
    );

    let after = snapshot(&mut context, &fixture).await;
    assert_account_content_eq(
        after.hoard.as_ref().expect("Hoard"),
        before.hoard.as_ref().expect("pre Hoard"),
    );
    assert_account_content_eq(
        after.recipient.as_ref().expect("recipient"),
        before.recipient.as_ref().expect("pre recipient"),
    );
    assert_eq!(
        custody_replay_revision(after.custody_replay.as_ref().expect("Claims-role replay")),
        CUSTODY_EXPECTED_REVISION,
        "no transfer means no cursor advance"
    );
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, losing),
        0
    );
    assert_eq!(
        lbv2_position_quantity(&after.actor_position.data, WINNER),
        ACTOR_CLAIMS[WINNERS],
        "the winning coordinate is untouched"
    );
    assert_eq!(
        lbv2_market_supply(&after.aggregate.data, losing),
        aggregate_claims()[0] - held
    );
}

/// The replay cursor is what refuses a second redemption of the same claims.
///
/// The first payout takes one of the wallet's two claims. The second names the
/// revisions the first left behind -- so the aggregate and the Position join
/// cleanly -- and carries the STALE Custody cursor. That is the double-spend
/// shape, and Custody is what stops it.
#[tokio::test]
async fn a_stale_custody_cursor_refuses_the_second_wallet_payout() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: replayed wallet payout",
    )
    .await;

    let first = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides {
            quantity: Some(1),
            ..WalletPayoutOverrides::default()
        },
        "claims rational-representation-v2: the first half of a wallet payout",
    )
    .await;
    if !first.accepted {
        eprintln!("first payout refusal logs:\n{}", first.logs.join("\n"));
    }
    assert!(first.accepted, "the first payout must commit");
    let before = snapshot(&mut context, &fixture).await;
    assert_eq!(
        custody_replay_revision(before.custody_replay.as_ref().expect("Claims-role replay")),
        CUSTODY_EXPECTED_REVISION + 1
    );

    let replayed = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides {
            quantity: Some(1),
            expected_market_revision: Some(1),
            expected_position_revision: Some(1),
            expected_custody_revision: Some(CUSTODY_EXPECTED_REVISION),
            ..WalletPayoutOverrides::default()
        },
        "claims rational-representation-v2: a wallet payout on a stale Custody cursor",
    )
    .await;
    assert!(
        !replayed.accepted,
        "a stale cursor must refuse: {:#?}",
        replayed.logs
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_account_content_eq(
        after.hoard.as_ref().expect("Hoard"),
        before.hoard.as_ref().expect("pre Hoard"),
    );
    assert_account_content_eq(&after.actor_position, &before.actor_position);
    assert_account_content_eq(&after.aggregate, &before.aggregate);
    assert_account_content_eq(
        after.custody_replay.as_ref().expect("Claims-role replay"),
        before
            .custody_replay
            .as_ref()
            .expect("pre Claims-role replay"),
    );
}

/// Every way a wallet payout can be bent, and the exact refusal for each.
///
/// One fixture, one prestate, nine substitutions. Each asserts the named code and
/// that the Hoard, the recipient, the wallet's Position and the aggregate are
/// byte-identical afterwards -- a refusal after a partial write would be worse
/// than an acceptance.
#[tokio::test]
async fn the_wallet_payout_hostiles_refuse_and_move_nothing() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: wallet payout hostiles",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;
    let stranger = context.payer.pubkey();
    assert_ne!(stranger, fixture.actor.pubkey());
    let another_partys_position = fixture.assets.get(WINNERS).expect("winner asset").position;

    for (overrides, code, label) in [
        (
            // The owner is named but did not sign. The frame spec's SIGNER pin
            // at coordinate 0 is what refuses, before any economics is read.
            WalletPayoutOverrides {
                authority_withholds_its_signature: true,
                ..WalletPayoutOverrides::default()
            },
            SignedDeltaSbfErrorV3::Accounts as u32,
            "the owner named but not signing",
        ),
        (
            // A signature from someone who is not the owner. Coordinate 0 must
            // BE `request.owner`, so this never reaches the Position.
            WalletPayoutOverrides {
                authority: Some(stranger),
                ..WalletPayoutOverrides::default()
            },
            ClaimsSbfError::Accounts as u32,
            "a signer who is not the Position owner",
        ),
        (
            // ... and naming that stranger as the owner too does not help. The
            // evaluator's Position join is what fires first: the offered
            // account's header says the actor owns it. Behind that stands
            // `build_candidates`, which would refuse the same frame again
            // because the canonical PDA under `(aggregate, stranger)` is not
            // this account -- two independent derivations of the same fact.
            WalletPayoutOverrides {
                authority: Some(stranger),
                owner: Some(stranger.to_bytes()),
                owner_pays_the_fee: true,
                ..WalletPayoutOverrides::default()
            },
            ClaimsSbfError::Economic as u32,
            "a stranger claiming to own this Position",
        ),
        (
            // Another party's Position at this same Market, offered with the
            // wallet's own signature. Same join, other direction: the account is
            // canonical for ITS owner, and its owner is not the signer.
            WalletPayoutOverrides {
                position_account: Some(another_partys_position),
                position: Some(another_partys_position.to_bytes()),
                ..WalletPayoutOverrides::default()
            },
            ClaimsSbfError::Economic as u32,
            "another party's Position at this Market",
        ),
        (
            // A substituted certificate: the request names a terminal receipt
            // the Core Market does not carry.
            WalletPayoutOverrides {
                terminal_record_digest: Some([0x5a; 32]),
                ..WalletPayoutOverrides::default()
            },
            ClaimsSbfError::Identity as u32,
            "a substituted certificate identity in the request",
        ),
        (
            // Keeping the request honest does not permit an account
            // substitution at the Resolution seam either.
            WalletPayoutOverrides {
                terminal_certificate_account: Some(sysvar::rent::ID),
                ..WalletPayoutOverrides::default()
            },
            ClaimsSbfError::Identity as u32,
            "a substituted certificate account",
        ),
        (
            // The certificate owner alone is not authority: the Resolution
            // ProgramData must be the deployment pinned by the release set.
            WalletPayoutOverrides {
                resolution_programdata_account: Some(fixture.core_programdata),
                ..WalletPayoutOverrides::default()
            },
            ClaimsSbfError::Release as u32,
            "a substituted Resolution ProgramData",
        ),
        (
            // Role `Claims` means THIS program is the executor. A caller program
            // coordinate naming anything else leaves an unauthenticated
            // executable in the frame, and is refused.
            WalletPayoutOverrides {
                caller_program: Some((TEST_CALLER_PROGRAM_ID, fixture.caller_programdata)),
                ..WalletPayoutOverrides::default()
            },
            SignedDeltaSbfErrorV3::Release as u32,
            "role Claims with a foreign caller program",
        ),
        (
            // The other crossing: an EXTERNAL role offering the owner's
            // signature where its own release-pinned PDA belongs.
            WalletPayoutOverrides {
                caller_role: Some(CallerRole::Trading),
                caller_program: Some((TEST_CALLER_PROGRAM_ID, fixture.caller_programdata)),
                ..WalletPayoutOverrides::default()
            },
            SignedDeltaSbfErrorV3::Release as u32,
            "role Trading with an owner signature at the authority coordinate",
        ),
    ] {
        let result = submit_wallet_payout(
            &mut context,
            &fixture,
            table,
            &addresses,
            overrides,
            &format!("claims rational-representation-v2: wallet payout with {label}"),
        )
        .await;
        assert_refused_with(&result, code, label);
        let after = snapshot(&mut context, &fixture).await;
        assert_account_content_eq(
            after.hoard.as_ref().expect("Hoard"),
            before.hoard.as_ref().expect("pre Hoard"),
        );
        assert_account_content_eq(
            after.recipient.as_ref().expect("recipient"),
            before.recipient.as_ref().expect("pre recipient"),
        );
        assert_account_content_eq(&after.actor_position, &before.actor_position);
        assert_account_content_eq(&after.aggregate, &before.aggregate);
        assert_account_content_eq(
            after.custody_replay.as_ref().expect("Claims-role replay"),
            before
                .custody_replay
                .as_ref()
                .expect("pre Claims-role replay"),
        );
    }
}

/// Core's receipt key is necessary but not sufficient: Claims independently
/// authenticates the exact live Resolution-owned certificate account.
#[tokio::test]
async fn the_resolution_certificate_owner_rent_width_and_body_are_all_required() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: Resolution certificate hostiles",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;
    let certificate_key = fixture
        .terminal_accounts
        .expect("terminal fixture")
        .certificate;
    let honest = observed(&mut context, certificate_key).await;
    let decoded = ResolutionCertificateV2::decode(&honest.data).expect("fixture certificate");

    let mut wrong_owner = honest.clone();
    wrong_owner.owner = CORE_PROGRAM_ID;

    let mut underfunded = honest.clone();
    underfunded.lamports = Rent::default()
        .minimum_balance(underfunded.data.len())
        .checked_sub(1)
        .expect("positive certificate rent minimum");

    let mut wrong_width = honest.clone();
    wrong_width.data.pop().expect("nonempty certificate");

    let mut wrong_body = honest.clone();
    let mut hostile_certificate = decoded;
    hostile_certificate.market = [0x8a; 32];
    wrong_body.data = hostile_certificate
        .to_bytes()
        .expect("canonical but cross-Market certificate")
        .to_vec();

    for (label, hostile) in [
        ("another owner", wrong_owner),
        ("less than rent exemption", underfunded),
        ("a truncated width", wrong_width),
        ("another Market in its canonical body", wrong_body),
    ] {
        context.set_account(&certificate_key, &AccountSharedData::from(hostile));
        let result = submit_wallet_payout(
            &mut context,
            &fixture,
            table,
            &addresses,
            WalletPayoutOverrides::default(),
            &format!("claims rational-representation-v2: certificate with {label}"),
        )
        .await;
        assert_refused_with(&result, ClaimsSbfError::Identity as u32, label);
        context.set_account(&certificate_key, &AccountSharedData::from(honest.clone()));
        assert_eq!(
            snapshot(&mut context, &fixture).await,
            before,
            "certificate hostile {label} must move no economic resource"
        );
    }
}

/// An UNRESOLVED Market pays nothing, at the Core join.
///
/// The Market is walked back to `Phase::Open` with its terminal receipt removed
/// -- through the bank, so the alteration actually reaches the chain -- and the
/// same request that just worked is refused. The Core state's own address is
/// derived from its identity, which does not move, so this is a phase refusal
/// and not an address one.
#[tokio::test]
async fn an_unresolved_market_pays_no_wallet_position() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: unresolved wallet payout",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    let resolved = observed(&mut context, fixture.market).await;
    let mut state = CoreState::decode(&resolved.data).expect("resolved Core state");
    assert_eq!(state.phase, CorePhase::Terminal);
    state.phase = CorePhase::Open;
    state.terminal_winner = 0;
    state.terminal_receipt = None;
    let mut unresolved = resolved.clone();
    unresolved.data = state.encode().expect("unresolved Core state").to_vec();
    context.set_account(&fixture.market, &AccountSharedData::from(unresolved));

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides::default(),
        "claims rational-representation-v2: a wallet payout against an unresolved Market",
    )
    .await;
    assert_refused_with(
        &result,
        ClaimsSbfError::Identity as u32,
        "an unresolved Market",
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_account_content_eq(
        after.hoard.as_ref().expect("Hoard"),
        before.hoard.as_ref().expect("pre Hoard"),
    );
    assert_account_content_eq(&after.actor_position, &before.actor_position);
}

/// A Position that belongs to ANOTHER Market's book is not payable here.
///
/// The wallet's own Position is forged in place -- through the bank, so the
/// alteration actually reaches the chain -- to carry a different
/// `market_account`. Everything else about the request is the one that just
/// worked: the same signer, the same canonical PDA, the same aggregate. What is
/// left is exactly the cross-Market join, which two independent readers make:
/// the evaluator's `validate_joins` (which fires first) and
/// `signed_delta_v3::build_candidates`.
#[tokio::test]
async fn a_cross_market_position_is_not_payable_here() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) = wallet_payout_lookup_table(
        &mut context,
        &fixture,
        "claims rational-representation-v2: cross-market wallet payout",
    )
    .await;
    let before = snapshot(&mut context, &fixture).await;

    let honest = observed(&mut context, fixture.actor_position).await;
    let mut forged = honest.clone();
    // The Core Market account: a real address on this chain, and emphatically
    // not this Claims aggregate. `LiabilityBasisPositionLayoutV2::MARKET` is
    // imported rather than restated so a layout move breaks here loudly.
    put(
        &mut forged.data,
        LiabilityBasisPositionLayoutV2::MARKET,
        fixture.market.as_ref(),
    );
    assert_ne!(forged.data, honest.data);
    context.set_account(&fixture.actor_position, &AccountSharedData::from(forged));

    // The BUILDER catches this before a transaction exists. `wallet_terminal_payout_v3`
    // decodes the Position it was handed and joins its Market against the
    // aggregate's, so a wallet running the real builder against this corrupted
    // account gets `Route` offline and never pays a fee. The campaign still
    // submits the hand-built frame below, because what the CHAIN does with a
    // frame no honest builder would emit is the property under test -- and
    // those are two different facts, which is why both are stated.
    let refusal_prestate =
        wallet_payout_prestate(&mut context, &fixture, fixture.actor_position).await;
    assert_eq!(
        wallet_payout_operator_report(
            &fixture,
            WalletPayoutOverrides {
                prestate_the_operator_refuses: true,
                ..WalletPayoutOverrides::default()
            },
            &refusal_prestate,
        )
        .err(),
        Some(WalletTerminalPayoutErrorV3::Route),
        "the wallet builder must refuse a cross-market Position offline",
    );

    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides {
            prestate_the_operator_refuses: true,
            ..WalletPayoutOverrides::default()
        },
        "claims rational-representation-v2: a wallet payout against a cross-market Position",
    )
    .await;
    assert_refused_with(
        &result,
        ClaimsSbfError::Economic as u32,
        "a Position from another Market's book",
    );
    let after = snapshot(&mut context, &fixture).await;
    assert_account_content_eq(
        after.hoard.as_ref().expect("Hoard"),
        before.hoard.as_ref().expect("pre Hoard"),
    );
    assert_account_content_eq(
        after.recipient.as_ref().expect("recipient"),
        before.recipient.as_ref().expect("pre recipient"),
    );
    assert_account_content_eq(&after.aggregate, &before.aggregate);
    assert_account_content_eq(
        after.custody_replay.as_ref().expect("Claims-role replay"),
        before
            .custody_replay
            .as_ref()
            .expect("pre Claims-role replay"),
    );
}

/// THE OTHER REDEMPTION ROUTE ALSO SURVIVES A STRANGER'S RETIREMENT.
///
/// `a_stranger_who_begins_retiring_cannot_end_a_holders_redemption` drives the
/// wallet payout, which submits a `TerminalSettlementRequestV3` straight to
/// Claims and is gated by `terminal_settlement_v3::authenticate_core`. It never
/// reaches `rational_product_v3::authenticate_core`, which is the phase gate on
/// the RationalRepresentation `RedeemTerminal` arm -- a fifth site with the same
/// defect and, until this test, the one welded site nobody had watched redeem in
/// `Retiring`.
///
/// A weld with an untested arm is how the original defect survived a design
/// document that named three files. So this drives the second route through the
/// same stranger's transition and asserts the same thing: the phase moved, the
/// holder was still paid, and the debits are the ones the Terminal-phase
/// redemption makes.
#[tokio::test]
async fn the_representation_redemption_also_survives_a_strangers_retirement() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let created = create_claims_custody_replay(&mut context, &fixture).await;
    assert!(
        created.accepted,
        "the Claims-role Custody replay must be creatable: {}",
        created.logs.join("\n")
    );

    let positive = wrapper_instruction(
        &fixture,
        RepresentationActionV2::RedeemTerminal,
        0,
        false,
        None,
        None,
    );
    let addresses = lookup_addresses(
        context.payer.pubkey(),
        fixture.actor.pubkey(),
        &[positive.clone()],
    );
    let (table, _) = create_live_lookup_table(
        &mut context,
        &addresses,
        "claims rational-representation-v2: retiring representation redemption",
    )
    .await;

    assert_eq!(
        core_phase(&mut context, fixture.market).await,
        CorePhase::Terminal
    );
    a_stranger_begins_retiring(&mut context, &fixture).await;
    assert_eq!(
        core_phase(&mut context, fixture.market).await,
        CorePhase::Retiring,
        "the stranger must actually have moved the phase, or this test proves nothing"
    );

    let before = snapshot(&mut context, &fixture).await;
    let accepted = submit_v0(
        &mut context,
        &fixture,
        positive,
        table,
        &addresses,
        "claims rational-representation-v2: representation redemption after a stranger retired",
    )
    .await
    .expect("terminal transaction");
    assert!(
        accepted.accepted,
        "the representation route must redeem in Retiring too:\n{}",
        accepted.logs.join("\n")
    );

    // Conservation, over acceptance: exactly the moves the Terminal-phase
    // redemption makes, and nothing the retirement could have changed.
    let after = snapshot(&mut context, &fixture).await;
    assert_eq!(replay_revision(&after.replay), 1);
    assert_eq!(lbv2_revision(&after.aggregate.data), 1);
    assert_eq!(
        lbv2_position_quantity(
            &after.positions.get(WINNERS).expect("winner Position").data,
            WINNER,
        ),
        CUSTODY_CLAIMS[WINNERS] - 1
    );
    assert_eq!(
        mint_supply(after.shard_mints.get(WINNERS).expect("winner Mint")),
        shard_supply(WINNERS) - DENOMINATOR
    );
    assert_eq!(
        token_amount(
            after
                .actor_shards
                .get(WINNERS)
                .expect("winner actor shards")
        ),
        actor_shards()[WINNERS] - DENOMINATOR
    );
    // Structured custody is untouched, exactly as in the Terminal-phase run:
    // the receipts it backs are still outstanding.
    assert_eq!(
        token_amount(
            after
                .structured_shards
                .get(WINNERS)
                .expect("winner structured shards"),
        ),
        structured_shards()[WINNERS]
    );
    assert_eq!(mint_supply(&after.receipt_mint), RECEIPT_SUPPLY);
    for index in 0..K {
        if index == WINNERS {
            continue;
        }
        assert_account_content_eq(
            after.shard_mints.get(index).expect("shard Mint"),
            before.shard_mints.get(index).expect("pre shard Mint"),
        );
    }
}
