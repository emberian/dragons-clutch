//! A stranger compacts a sleeping fractional holder's reserve, end to end.
//!
//! Design `docs/design/CLAIM_CHECK_COMPACTION_V1.md` §17. Six lanes built the
//! route (FRACCHECK-6's `8cb9e6c5`), its receipt (`a18760c4`), the frame guards
//! (`a02082cb`) and the test signer that can produce the capability root's
//! signature (`7e81ce55`) -- and every one of them refused to call it driven,
//! because "green in a scratchpad" is what §17.6's evidence warns about. This
//! file is the drive.
//!
//! # What this campaign is for
//!
//! A market resolves. One holder never comes back. Their collateral sits in a
//! reserve Position owned by a Trading PDA that cannot sign for a payout, and
//! the market can never finish retiring while it stands. Compaction is the
//! permissionless crank that resolves it: a stranger -- nobody's agent, holding
//! no authority -- moves the collateral into an escrow vault, writes a
//! claim-check the shard holders can redeem against whenever they return, and
//! closes the Position. The holder loses nothing; the market gets to finish.
//!
//! # The fixture is faithful where the last one was not, and that is the point
//!
//! FRACCHECK-6 named three gaps it had to leave open, and all three are closed
//! here rather than worked around:
//!
//! - **The RentCredit is real.** `tests/fractional_atomic.rs` plants
//!   `RENT_CREDIT` as `add_account(.., system_program::ID, Vec::new())` -- a
//!   bare lamport sink with no record in it. The compaction route decodes a
//!   genuine `LifecycleRentCreditV2`, so this campaign plants one, at its
//!   derived address, under the real `dclutch_rent_sbf.so`. The fixture was
//!   unfaithful; the route was right.
//! - **The reserve's admission exists.** A `ProtocolPositionAdmissionV2` whose
//!   `owner_kind` is `TradingRecord` and whose `position_owner` is the
//!   capability root existed nowhere in the repository -- `plant_admission_of_kind`
//!   hard-codes the actor as owner. This campaign writes one, because that
//!   record is what carries the owner kind the whole route exists to admit.
//! - **The ruled fiftieth account is exercised.** WAVE `b4546291` put the Rent
//!   program in the frame so `authenticate_rent_credit` could run. It runs here,
//!   against a credit the admission names, and the hostile below proves a
//!   credit at a non-derived address is refused by name.
//!
//! # What is real and what is a stand-in
//!
//! Real: every `.so` -- Claims, Registry, Core, Custody, Rent, Token-2022 (the
//! audited v11 fixture, digest-gated by `run-program-test.sh`) and the test
//! signer. Real: the 50-account frame, exactly as `FractionalCompactionRoleV1`
//! declares it, built from the declaration rather than written out.
//!
//! The stand-in is the same one `fractional-compaction-caller` documents and
//! cannot avoid: `invoke_signed` signs only for the calling program's own
//! addresses, so the capability root here is derived under the test signer
//! rather than under Trading. What is under test is the Claims route given a
//! correctly-signed root, not Trading's derivation of it -- which
//! `fractional_root_signer`'s own witnesses cover. The two halves meet in the
//! design, not in one test.
//!
//! No address lookup table, and that is deliberate rather than an omission. The
//! frame declaration notes this route serialises through the ALT the fractional
//! campaigns use on a real cluster, where the 1,232-byte packet limit binds. A
//! `ProgramTest` bank enforces the 64-account lock limit and not the packet
//! size, so a 51-account legacy transaction is what the sibling terminal
//! campaign already sends and is what this one sends. The ALT is a transport
//! concern and this campaign is about the route.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use std::{env, fs, path::PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD};

use dclutch_capability_program_contract::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_claims_svm::{
    CallerRole,
    claim_check_request_v1::OpenClaimCheckEscrowRequestV1,
    claim_check_v1::{
        COMPACTION_DEADLINE_SLOTS_V1, ClaimCheckEscrowSeedsV1, ClaimCheckEscrowV1,
        ClaimCheckVaultSeedsV1,
    },
    protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionAdmissionEvidenceV2,
        ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
        ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1,
    CustodyReplaySeedsV1, CustodyVaultSeedsV1,
};
use dclutch_fractional_atomic_program_test::{
    campaign_support::{
        ReleaseSetInputV1, activation_cache, add_account, add_account_with_lamports, add_finalized,
        add_upgradeable_program, collateral_mint_bytes, finalized, mint_bytes, mint_supply,
        token_account_bytes_for, token_amount, token_program_id,
    },
    narrow_fixture::{
        NarrowBasisInputV3, NarrowFixtureInputV2, NarrowFixtureV2, NarrowRecordV2,
        NarrowSplineBasisInputV3, NarrowTerminalInputV2, compile_narrow_fixture_v2,
        compile_narrow_fixture_v3,
    },
};
use dclutch_fractional_claim_contract::{
    FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4, FractionalRootInputV1, FractionalRootV1,
};
use dclutch_fractional_claim_kernel::{
    FractionalExposureTermsInputV2, encode_fractional_exposure_terms_v2,
    fractional_exposure_terms_bytes_v2,
};
use dclutch_fractional_cubic_life_evidence::{
    AccountImageV1, COMPACTION_SCHEMA_V1, CompactionBridgeV1, PreterminalBridgeV1, canonical_bytes,
    digest as bridge_digest, read_preterminal, write_atomic,
};
use dclutch_product_payoff_v2_codec::{
    price_gate_v1::{
        PRICE_GATE_ATOM_COUNT_OFFSET_V1, PRICE_GATE_DEGREE_OFFSET_V1,
        PRICE_GATE_DENOMINATORS_OFFSET_V1, PRICE_GATE_MAGIC_OFFSET_V1, PRICE_GATE_MAGIC_V1,
        PRICE_GATE_MASS_OFFSET_V1, PRICE_GATE_NUMERATORS_OFFSET_V1, PRICE_GATE_PRICES_OFFSET_V1,
        PRICE_GATE_PROFILE_OFFSET_V1, PRICE_GATE_PROFILE_V1, PRICE_GATE_REQUEST_BYTES_V1,
        PRICE_GATE_SCALE_OFFSET_V1, PRICE_GATE_SCHEMA_VERSION_V1, PRICE_GATE_VERSION_OFFSET_V1,
        PRICE_GATE_WEIGHTS_OFFSET_V1, PRICE_GATE_WIDTH_OFFSET_V1,
    },
    runtime_v3::ProductBasisV3,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
use dclutch_rent_contract::RefundAuthority;
use dclutch_rent_contract::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
};
use dclutch_resolution_codec::{ResolutionCertificateKindV2, ResolutionCertificateV2};
use dclutch_token_svm::{
    PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
    TokenBehaviorSelectionV2,
};
use solana_account::Account;
use dclutch_program_test_evidence::TransactionEvidence;
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar,
};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{signature::Keypair, signer::Signer};
use solana_sdk_ids::system_program;
use solana_transaction::Transaction;

// ---------------------------------------------------------------------------
// Identities
// ---------------------------------------------------------------------------

const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa1; 32]);
const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa2; 32]);
const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa3; 32]);
const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa4; 32]);
/// The Rent program: the ruled fiftieth account (WAVE `b4546291`).
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xb6; 32]);
/// The test signer that derives and signs the Fractional capability root.
const CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa9; 32]);

const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0x74; 32]);
const CERTIFICATE_ACCOUNT: Pubkey = Pubkey::new_from_array([0x86; 32]);
const CUSTODY_CONTEXT: [u8; 32] = [0x62; 32];
const GRAPH_ID: [u8; 32] = [0x7c; 32];
const EXPOSURE_ID: [u8; 32] = [0x7a; 32];
const RESOLUTION_POLICY: [u8; 32] = [0x51; 32];
const REFUND_WALLET: [u8; 32] = [0x71; 32];

const GENERATION: u64 = 37;
const ROOT_REVISION: u64 = 1;
const DENOMINATOR: u64 = 10;
const POSITION_REVISION: u64 = 3;
const OUTCOME: u32 = 0;
const MINT_DECIMALS: u8 = 0;
const COLLATERAL_DECIMALS: u8 = 6;
const TERMINAL_WIDTH: usize = 8;
const CURVED_WIDTH: usize = 3;
const CURVED_PAYOUT_SCALE: u64 = 7;
const CUBIC_WIDTH: usize = 4;
const CUBIC_PAYOUT_SCALE: u64 = 11;
const CURVED_RESULT_NUMERATOR: i128 = 3;
const CURVED_RESULT_DENOMINATOR: u64 = 2;
const CURVED_OUTCOME: u32 = 1;

/// Native Claims the sleeping holder's reserve locked at the coordinate.
const RESERVE_NATIVE_CLAIMS: u64 = 4;
/// Claims the actor holds outside the reserve.
const ACTOR_FUNDED_BALANCE: u64 = 1_000;
/// Shards outstanding against the reserve.
const OUTSTANDING_SHARDS: u64 = RESERVE_NATIVE_CLAIMS * DENOMINATOR;
/// Collateral the market's hoard holds before the crank.
const HOARD_ATOMS: u64 = 10_000;
const HOLDER_SHARD_TOKENS: Pubkey = Pubkey::new_from_array([0xc1; 32]);
const HOLDER_COLLATERAL_TOKENS: Pubkey = Pubkey::new_from_array([0xc2; 32]);
const CRANKER_COLLATERAL_TOKENS: Pubkey = Pubkey::new_from_array([0xc3; 32]);

/// The sleeping holder. Funded, and never signs anything in this campaign.
fn holder_keypair() -> Keypair {
    Keypair::new_from_array([0x5c; 32])
}

/// The original wrapper/author. It transfers the represented balance to the
/// sleeping holder and is not the holder the permissionless crank compacts.
fn actor_keypair() -> Keypair {
    Keypair::new_from_array([0x2c; 32])
}

fn propagated_preterminal() -> Option<PreterminalBridgeV1> {
    let raw = env::var_os("DCLUTCH_CUBIC_PRETERMINAL_BRIDGE")?;
    let path = PathBuf::from(raw);
    assert!(
        path.is_absolute(),
        "preterminal bridge path must be absolute"
    );
    Some(read_preterminal(&path).expect("strict preterminal bridge"))
}

fn hex_digest(bytes: &[u8]) -> String {
    hash(bytes)
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The party who opens the escrow and advances its rent.
///
/// A DIFFERENT key from the holder, and the campaign would be worthless if it
/// were not. The whole claim is that somebody with no relationship to the
/// position can resolve it, so a campaign whose opener was the holder would
/// prove only that an owner can act on their own account.
fn opener_keypair() -> Keypair {
    Keypair::new_from_array([0x3f; 32])
}

/// The party who cranks the compaction.
///
/// A THIRD key, distinct from both the holder and the opener, and the
/// distinctness is load-bearing twice over. It proves the crank is open to
/// somebody who did not pay to open the escrow -- otherwise "permissionless"
/// would only have been shown for the one party already invested in the market.
/// And it keeps the conservation table legible: the plan deliberately folds
/// aliased sinks, so a cranker who was also the opener would collapse two
/// lamport flows into one row and hide which rule paid which.
fn cranker_keypair() -> Keypair {
    Keypair::new_from_array([0x4d; 32])
}

// ---------------------------------------------------------------------------
// Artifacts
// ---------------------------------------------------------------------------

struct Artifacts {
    claims: Vec<u8>,
    registry: Vec<u8>,
    core: Vec<u8>,
    custody: Vec<u8>,
    rent: Vec<u8>,
    caller: Vec<u8>,
    token: Vec<u8>,
}

fn artifacts() -> Artifacts {
    let directory = PathBuf::from(env::var("SBF_OUT_DIR").expect("SBF_OUT_DIR is required"));
    let read = |name: &str| {
        let path = directory.join(name);
        assert!(path.is_file(), "missing real ELF: {}", path.display());
        fs::read(path).expect("read real ELF")
    };
    Artifacts {
        claims: read("dclutch_claims_sbf.so"),
        registry: read("dclutch_registry_sbf.so"),
        core: read("dclutch_core_sbf.so"),
        custody: read("dclutch_custody_sbf.so"),
        rent: read("dclutch_rent_sbf.so"),
        caller: read("dclutch_fractional_compaction_test_caller_sbf.so"),
        token: read("spl_token_2022.so"),
    }
}

fn selection_config_digest(terms_bytes: &[u8]) -> [u8; 32] {
    use dclutch_fractional_claim_kernel::{
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_SELECTION_CONFIG_BYTES_V1,
        FractionalExposureTermsAdmissionV2, FractionalExposureTermsV2,
        encode_fractional_selection_config_v1, fractional_selection_config_from_terms_v1,
    };
    let terms_digest: [u8; 32] = hash(terms_bytes).to_bytes();
    let terms = FractionalExposureTermsV2::decode(
        terms_bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: terms_digest,
            finalized_terms_id: terms_digest,
            recomputed_terms_digest: terms_digest,
            finalized_terms_digest: terms_digest,
            record_authenticated: true,
        },
    )
    .expect("campaign terms decode");
    let mut config = [0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(
        fractional_selection_config_from_terms_v1(terms),
        &mut config,
    )
    .expect("campaign selection config");
    hash(&config).to_bytes()
}

fn terminal_certificate_bytes(
    basis_profile: CampaignBasisV1,
    winner: u32,
    core_market: Pubkey,
    product_record_digest: [u8; 32],
) -> Vec<u8> {
    ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: core_market.to_bytes(),
        route: [0x87; 32],
        source_material: RESOLUTION_POLICY,
        product_record_digest,
        provider_evidence: [0x88; 32],
        funding_allocation: [0; 32],
        receipt_account: CERTIFICATE_ACCOUNT.to_bytes(),
        generation: GENERATION,
        attempt_index: 0,
        schedule_index: 0,
        selector: winner,
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: match basis_profile {
            CampaignBasisV1::Categorical => 1,
            CampaignBasisV1::CurvedDegreeTwo | CampaignBasisV1::CurvedDegreeThree => {
                CURVED_RESULT_NUMERATOR
            }
        },
        result_denominator: match basis_profile {
            CampaignBasisV1::Categorical => 1,
            CampaignBasisV1::CurvedDegreeTwo | CampaignBasisV1::CurvedDegreeThree => {
                CURVED_RESULT_DENOMINATOR
            }
        },
        observed_at: 1,
    }
    .to_bytes()
    .expect("canonical Resolution certificate")
    .to_vec()
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

/// Everything the campaign needs to name after the market is planted.
struct CampaignFixture {
    plant: CampaignPlantV1,
    shared: NarrowFixtureV2,
    release_set: [u8; 32],
    activation_cache: Pubkey,
    realm_record: NarrowRecordV2,
    terms_record: NarrowRecordV2,
    behavior_record: NarrowRecordV2,
    root: Pubkey,
    shard_mint: Pubkey,
    holder_shard_tokens: Pubkey,
    holder_collateral_tokens: Pubkey,
    cranker_collateral_tokens: Pubkey,
    holder: Pubkey,
    opener: Pubkey,
    cranker: Pubkey,
    custody_replay: Pubkey,
    custody_authority: Pubkey,
    hoard: Pubkey,
    /// The market's RentCredit, at its derived address under the Rent program.
    rent_credit: Pubkey,
    /// The reserve Position's admission record.
    reserve_admission: Pubkey,
    escrow: Pubkey,
    vault: Pubkey,
    /// The coordinate the market resolved to.
    winner: u32,
    /// ProductBasisV3 family this campaign executes.
    basis_profile: CampaignBasisV1,
    /// Representation coordinate held by the sleeping holder.
    outcome: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CampaignBasisV1 {
    Categorical,
    CurvedDegreeTwo,
    CurvedDegreeThree,
}

impl CampaignBasisV1 {
    const fn width(self) -> usize {
        match self {
            Self::Categorical => TERMINAL_WIDTH,
            Self::CurvedDegreeTwo => CURVED_WIDTH,
            Self::CurvedDegreeThree => CUBIC_WIDTH,
        }
    }

    const fn outcome(self) -> u32 {
        match self {
            Self::Categorical => OUTCOME,
            Self::CurvedDegreeTwo | Self::CurvedDegreeThree => CURVED_OUTCOME,
        }
    }

    const fn degree(self) -> Option<u8> {
        match self {
            Self::Categorical => None,
            Self::CurvedDegreeTwo => Some(2),
            Self::CurvedDegreeThree => Some(3),
        }
    }

    const fn payout_scale(self) -> u64 {
        match self {
            Self::Categorical => 1,
            Self::CurvedDegreeTwo => CURVED_PAYOUT_SCALE,
            Self::CurvedDegreeThree => CUBIC_PAYOUT_SCALE,
        }
    }
}

/// What this fixture plants wrong on purpose, if anything.
///
/// Each knob changes one joined fact after every outer compaction guard has a
/// valid fixture. Everything else about a hostile run is IDENTICAL to the
/// admitted one -- same market, same records, same frame, same request --
/// because a hostile that differs in more than the fact under test proves only
/// that two different things differ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CampaignPlantV1 {
    /// The faithful market.
    Faithful,
    /// A RentCredit whose persisted bump does not re-derive its own address.
    ///
    /// The record still decodes, and it still carries this market's own market,
    /// release set and generation -- so every conjunct FRACCHECK-6's route
    /// already made still passes. The ONLY thing wrong with it is the one thing
    /// the ruled account made checkable: `create_program_address` over the
    /// record's own seeds no longer reproduces the address it is sitting at.
    /// Planted at the admission's pinned address, so the two pins pass too and
    /// the derivation is the sole remaining discriminator.
    RentCreditAtANonDerivedAddress,
    /// The terminal certificate decodes but names another Market.
    TerminalCertificateIdentityMismatch,
    /// SignedDelta's caller role coordinates name Trading under a Claims plan.
    SignedDeltaCallerReleaseMismatch,
    /// The exposure terms are not the terms this Market's manifest selected.
    ///
    /// The faithful fixture derives the capability root header's selected
    /// config FROM the terms it plants (`selection_config_digest`), so the two
    /// agree by construction and there is exactly one field to move to make
    /// them disagree. This plant moves the header's, which is the cheap half;
    /// the ATTACK moves the other half, and they are the same disagreement.
    ///
    /// The attack, stated because a reader should not have to reconstruct it
    /// from a plant name. Registry publication is permissionless, and before
    /// `authenticate_selected_config` the route's only joins on the terms were
    /// `market()` and `release_set()` -- fields the record's own author writes.
    /// So a cranker publishes a second well-formed `FractionalExposureTermsV2`
    /// naming this Market, this release set and this same shard Mint, with
    /// `denominator = 40` instead of 10, and names a rate of 16 instead of 4.
    /// The conservation plan is satisfied exactly -- `(40 / 40) * 16 == 16`, the
    /// entitlement the terminal derivation actually moved -- and the record that
    /// gets written promises every returning holder `floor(shards / 40) * 16`.
    /// The sleeping holder's 40 shards still redeem 16. A holder who had been
    /// transferred 20 of them redeems ZERO, forever, and the collateral stays in
    /// the vault. That is the redeemer-chooses-the-matrix shape of `a968858c`,
    /// on the one route in this family a stranger may turn.
    UnselectedExposureTerms,
}

fn campaign_fixture() -> (ProgramTest, CampaignFixture) {
    campaign_fixture_planting(CampaignPlantV1::Faithful)
}

fn campaign_fixture_planting(plant: CampaignPlantV1) -> (ProgramTest, CampaignFixture) {
    campaign_fixture_full(plant, OUTCOME)
}

/// `winner` is the coordinate the market resolved to. When it is not the
/// reserve's own coordinate, the reserve is worthless and the whole compaction
/// runs on the zero-payout side of every weld.
fn campaign_fixture_full(plant: CampaignPlantV1, winner: u32) -> (ProgramTest, CampaignFixture) {
    campaign_fixture_basis(plant, CampaignBasisV1::Categorical, winner)
}

fn campaign_fixture_basis(
    plant: CampaignPlantV1,
    basis_profile: CampaignBasisV1,
    winner: u32,
) -> (ProgramTest, CampaignFixture) {
    let artifacts = artifacts();
    let propagated = if plant == CampaignPlantV1::Faithful
        && basis_profile == CampaignBasisV1::CurvedDegreeThree
    {
        propagated_preterminal()
    } else {
        None
    };
    if let Some(bridge) = &propagated {
        for (name, expected, observed) in [
            (
                "Claims",
                bridge.elves.claims.as_str(),
                hex_digest(&artifacts.claims),
            ),
            (
                "Registry",
                bridge.elves.registry.as_str(),
                hex_digest(&artifacts.registry),
            ),
            (
                "Core",
                bridge.elves.core.as_str(),
                hex_digest(&artifacts.core),
            ),
            (
                "Custody",
                bridge.elves.custody.as_str(),
                hex_digest(&artifacts.custody),
            ),
            (
                "Rent",
                bridge.elves.rent.as_str(),
                hex_digest(&artifacts.rent),
            ),
            (
                "Trading",
                bridge.elves.trading.as_str(),
                hex_digest(&artifacts.caller),
            ),
            (
                "Token-2022",
                bridge.elves.token_2022.as_str(),
                hex_digest(&artifacts.token),
            ),
        ] {
            assert_eq!(observed, expected, "{name} ELF substitution");
        }
    }
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    for (name, program, elf) in [
        (
            "dclutch_claims_sbf",
            CLAIMS_PROGRAM_ID,
            artifacts.claims.as_slice(),
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
            "dclutch_custody_sbf",
            CUSTODY_PROGRAM_ID,
            artifacts.custody.as_slice(),
        ),
        (
            "dclutch_rent_sbf",
            RENT_PROGRAM_ID,
            artifacts.rent.as_slice(),
        ),
        (
            "dclutch_fractional_compaction_test_caller_sbf",
            CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
        (
            "spl_token_2022",
            token_program_id(),
            artifacts.token.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }

    // The Trading role binds the COMPACTION caller, not the atomic one: the
    // capability root must be derivable by whichever program has to
    // `invoke_signed` for it, and here that is the test signer.
    let (release_set, cache_bytes) = activation_cache(&ReleaseSetInputV1 {
        core: (CORE_PROGRAM_ID, artifacts.core.as_slice()),
        claims: (CLAIMS_PROGRAM_ID, artifacts.claims.as_slice()),
        trading: (CALLER_PROGRAM_ID, artifacts.caller.as_slice()),
        custody: Some((CUSTODY_PROGRAM_ID, artifacts.custody.as_slice())),
    });
    if let Some(bridge) = &propagated {
        assert_eq!(release_set, bridge.release_set, "release-set substitution");
    }
    let activation_cache_key = Pubkey::find_program_address(
        &[
            dclutch_registry_contract::ACTIVATION_PDA_DOMAIN_V1,
            &release_set,
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        activation_cache_key,
        REGISTRY_PROGRAM_ID,
        cache_bytes,
    );

    let adapter = PRODUCTION_ADAPTER_RELEASES
        .get(1)
        .copied()
        .expect("Token-2022 production adapter");
    let realm_bytes = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: COLLATERAL_MINT.to_bytes(),
        collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("canonical Realm")
    .to_bytes()
    .to_vec();
    let realm_record = finalized(REGISTRY_PROGRAM_ID, REALM_SCHEMA_RELEASE_ID_V1, realm_bytes);
    let realm_id = realm_record.digest;
    if let Some(bridge) = &propagated {
        assert_eq!(realm_id, bridge.realm, "Realm substitution");
    }

    let actor = actor_keypair().pubkey();
    let holder = holder_keypair().pubkey();
    let opener = opener_keypair().pubkey();
    let cranker = cranker_keypair().pubkey();

    // Probe compile: the Core market address is needed to derive the RentCredit
    // before the real compile can name it as the rent beneficiary.
    let compile = |reserve_owner: Pubkey, rent_beneficiary: Pubkey| {
        let input = NarrowFixtureInputV2 {
            outcome_count: basis_profile.width(),
            funded_coordinate: basis_profile.outcome() as usize,
            registry_program: REGISTRY_PROGRAM_ID,
            core_program: CORE_PROGRAM_ID,
            claims_program: CLAIMS_PROGRAM_ID,
            release_set,
            realm_id,
            custody_context: CUSTODY_CONTEXT,
            generation: GENERATION,
            actor_owner: actor,
            reserve_owner,
            funded_balance: ACTOR_FUNDED_BALANCE - RESERVE_NATIVE_CLAIMS,
            reserve_balance: RESERVE_NATIVE_CLAIMS,
            position_revision: POSITION_REVISION,
            terminal: Some(NarrowTerminalInputV2 {
                winner,
                receipt: CERTIFICATE_ACCOUNT.to_bytes(),
            }),
            rent_beneficiary,
            graph_id: GRAPH_ID,
            exposure_id: EXPOSURE_ID,
        };
        match basis_profile {
            CampaignBasisV1::Categorical => compile_narrow_fixture_v2(input),
            CampaignBasisV1::CurvedDegreeTwo => {
                let knots = [0_i128, 0, 0, 3, 3, 3];
                let failure_payouts = [0_u64, 0, CURVED_PAYOUT_SCALE];
                let price_gate = curved_price_gate_certificate(basis_profile);
                compile_narrow_fixture_v3(
                    input,
                    NarrowBasisInputV3::SplineDegree2To3(NarrowSplineBasisInputV3 {
                        degree: 2,
                        interior_multiplicity: false,
                        payout_scale: CURVED_PAYOUT_SCALE,
                        knot_denominator: 1,
                        knots: &knots,
                        failure_payouts: &failure_payouts,
                        price_gate_certificate: &price_gate,
                    }),
                )
            }
            CampaignBasisV1::CurvedDegreeThree => {
                let knots = [0_i128, 0, 0, 0, 3, 3, 3, 3];
                let failure_payouts = [0_u64, 0, 0, CUBIC_PAYOUT_SCALE];
                let price_gate = curved_price_gate_certificate(basis_profile);
                compile_narrow_fixture_v3(
                    input,
                    NarrowBasisInputV3::SplineDegree2To3(NarrowSplineBasisInputV3 {
                        degree: 3,
                        interior_multiplicity: false,
                        payout_scale: CUBIC_PAYOUT_SCALE,
                        knot_denominator: 1,
                        knots: &knots,
                        failure_payouts: &failure_payouts,
                        price_gate_certificate: &price_gate,
                    }),
                )
            }
        }
        .expect("narrow terminal fixture")
    };
    let probe = compile(
        Pubkey::new_from_array([0xef; 32]),
        Pubkey::new_from_array([0xee; 32]),
    );
    let core_market = probe.core_market;

    // THE REAL RENT CREDIT, at its derived address under the real Rent program.
    // The address is what `authenticate_rent_credit` re-derives from the
    // record's own persisted seeds, so the bump that derivation produced is fed
    // back into the record -- a fixture whose bump disagreed with its address
    // would be a record no Rent program could ever have written.
    let (rent_credit, rent_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            core_market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );
    // The bump is what the derivation re-runs. A wrong one leaves a record that
    // decodes, matches this market, and sits at the address the admission pins
    // -- and still cannot be the credit the Rent program derived.
    let planted_bump = match plant {
        CampaignPlantV1::Faithful
        | CampaignPlantV1::TerminalCertificateIdentityMismatch
        | CampaignPlantV1::SignedDeltaCallerReleaseMismatch
        | CampaignPlantV1::UnselectedExposureTerms => rent_bump,
        CampaignPlantV1::RentCreditAtANonDerivedAddress => rent_bump.wrapping_sub(1),
    };
    let rent_credit_bytes = LifecycleRentCreditV2::new(
        RefundAuthority::new(REFUND_WALLET).expect("refund authority"),
        LifecycleAccountIdV2::new(core_market.to_bytes()).expect("Market"),
        LifecycleAccountIdV2::new(release_set).expect("release set"),
        GENERATION,
        planted_bump,
    )
    .expect("lifecycle RentCredit")
    .to_bytes()
    .to_vec();

    let shard_mints: Vec<[u8; 32]> = (0..basis_profile.width())
        .map(|index| {
            let mut bytes = [0x77_u8; 32];
            let index = u32::try_from(index).expect("representation coordinate");
            bytes[0..4].copy_from_slice(&index.to_le_bytes());
            bytes
        })
        .collect();
    let shard_mint = Pubkey::new_from_array(
        shard_mints[usize::try_from(basis_profile.outcome()).expect("outcome")],
    );
    let behavior_record = finalized(
        REGISTRY_PROGRAM_ID,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        TokenBehaviorSelectionV2::new(realm_id, release_set)
            .expect("token behavior selection")
            .to_bytes()
            .to_vec(),
    );
    let terms_width = fractional_exposure_terms_bytes_v2(shard_mints.len()).expect("terms width");
    let mut terms_scratch = vec![0_u8; terms_width];
    let mut terms_bytes = vec![0_u8; terms_width];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: core_market.to_bytes(),
            product_record: probe.product.digest,
            result_domain: probe.result_domain.digest,
            release_set,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: behavior_record.digest,
            exposure_id: EXPOSURE_ID,
            product_basis: probe.linked_basis.digest,
            representation_basis: probe.semantic_basis_id,
            graph_id: GRAPH_ID,
            product_width: probe.outcome_count,
            denominator: DENOMINATOR,
            shard_mints: &shard_mints,
        },
        &mut terms_scratch,
        &mut terms_bytes,
    )
    .expect("exact Fractional exposure terms");
    let terms_record = finalized(
        REGISTRY_PROGRAM_ID,
        dclutch_fractional_claim_kernel::FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        terms_bytes,
    );

    // The Market's manifest-selected config. Derived from the terms it plants,
    // so the faithful fixture agrees by construction and the hostile has one
    // field to move.
    let selected_config = match plant {
        CampaignPlantV1::UnselectedExposureTerms => [0x9c; 32],
        _ => selection_config_digest(&terms_record.bytes),
    };
    let selection = CapabilityExecutionSelectionV1::new(
        0,
        ContentId::new([0x81; 32]).expect("manifest"),
        ContentId::new(dclutch_fractional_claim_contract::FRACTIONAL_CAPABILITY_KIND_ID_V1)
            .expect("kind"),
        ContentId::new([0x83; 32]).expect("capability release"),
        ContentId::new(selected_config).expect("config"),
    )
    .expect("capability execution selection");
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set).expect("release set"),
        core_market.to_bytes(),
        GENERATION,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .expect("capability root header");
    let (root, root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &CALLER_PROGRAM_ID);

    // The real compile: the reserve belongs to the root, and the market's rent
    // beneficiary is the derived credit -- so Core state and the Position's
    // admission name the same account, as a founded market's would.
    let shared = compile(root, rent_credit);
    assert_eq!(shared.core_market, core_market);
    if let Some(bridge) = &propagated {
        assert_eq!(bridge.market, core_market.to_bytes(), "Market substitution");
        assert_eq!(
            bridge.aggregate,
            shared.claims_market.to_bytes(),
            "aggregate substitution"
        );
        assert_eq!(
            bridge.product, shared.product.digest,
            "Product substitution"
        );
        assert_eq!(
            bridge.product_basis, shared.linked_basis.digest,
            "ProductBasis substitution"
        );
        assert_eq!(bridge.terms, terms_record.digest, "terms substitution");
        assert_eq!(bridge.root, root.to_bytes(), "root substitution");
        assert_eq!(
            bridge.shard_mint,
            shard_mint.to_bytes(),
            "shard Mint substitution"
        );
        assert_eq!(
            bridge.holder,
            holder.to_bytes(),
            "sleeping holder substitution"
        );
        assert_eq!(
            bridge.holder_shard_token,
            HOLDER_SHARD_TOKENS.to_bytes(),
            "holder shard account substitution"
        );
        assert_eq!(bridge.denominator, DENOMINATOR);
        assert_eq!(bridge.outstanding_shards, OUTSTANDING_SHARDS);
        assert_eq!(bridge.reserve_native_claims, RESERVE_NATIVE_CLAIMS);
    }

    let root_state = FractionalRootV1::new(FractionalRootInputV1 {
        bump: root_bump,
        terms: terms_record.digest,
        market: core_market.to_bytes(),
        rent_beneficiary: holder.to_bytes(),
        revision: ROOT_REVISION,
        historical_rent_principal: 1,
    })
    .expect("fractional root state");
    let mut root_bytes = vec![0_u8; FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4];
    root_bytes.copy_from_slice(&header.to_bytes());
    root_bytes.extend_from_slice(&root_state.to_bytes());

    for record in [
        &shared.product,
        &shared.result_domain,
        &shared.portfolio,
        &shared.linked_basis,
        &shared.exposure,
        &terms_record,
        &behavior_record,
        &realm_record,
    ] {
        add_finalized(&mut test, record);
    }
    add_account(
        &mut test,
        shared.core_market,
        CORE_PROGRAM_ID,
        shared.core_state.clone(),
    );
    add_account(
        &mut test,
        shared.claims_market,
        CLAIMS_PROGRAM_ID,
        shared.claims_market_bytes.clone(),
    );

    // THE RESERVE POSITION AND ITS ADMISSION. The admission is the artifact
    // that existed nowhere in this repository: `owner_kind = TradingRecord`
    // with `position_owner` the capability root. It is planted with exactly
    // what Admit would have written, because the compaction route
    // authenticates it and reads the owner kind that admits this whole route.
    let position_principal = Rent::default().minimum_balance(shared.reserve_position.bytes.len());
    let admission_principal = Rent::default().minimum_balance(
        dclutch_claims_svm::protocol_position_v2::PROTOCOL_POSITION_ADMISSION_BYTES_V2,
    );
    // Deliberately not the rent minimum. A fixture pinned to the minimum
    // silently excuses a route that recomputes the minimum instead of reading
    // the lamports actually there -- and the sweep this campaign measures is
    // precisely a question about lamports actually there.
    let position_lamports = position_principal + 13;
    let admission_lamports = admission_principal + 11;
    let reserve_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(shared.claims_market.to_bytes(), root.to_bytes())
            .expect("reserve admission seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let admission = ProtocolPositionAdmissionV2::new(
        ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
            presence: ProtocolPositionPresenceV2::Vacant,
            release_set,
            market: core_market.to_bytes(),
            position_owner: root.to_bytes(),
            parent_request_digest: [0x77; 32],
            // The two pins the ruled fiftieth account authenticates against.
            rent_credit: rent_credit.to_bytes(),
            rent_program: RENT_PROGRAM_ID.to_bytes(),
            generation: GENERATION,
            expected_market_revision: 0,
            expected_position_revision: 0,
            observed_position_lamports: position_lamports,
            observed_admission_lamports: admission_lamports,
            position_rent_principal: position_principal,
            admission_rent_principal: admission_principal,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        },
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: shared.product.digest,
            semantic_basis_id: shared.semantic_basis_id,
            linked_basis_record_digest: shared.linked_basis.digest,
            request_digest: [0x78; 32],
            claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
            trading_program: CALLER_PROGRAM_ID.to_bytes(),
            capability_descriptor: [0; 32],
            capability_outcome: 0,
            outcome_count: u32::try_from(basis_profile.width()).expect("width"),
        },
    )
    .expect("the reserve's installed admission");
    for position in shared.ordered_positions() {
        let lamports = if position.owner == root {
            position_lamports
        } else {
            Rent::default().minimum_balance(position.bytes.len())
        };
        add_account_with_lamports(
            &mut test,
            position.account,
            CLAIMS_PROGRAM_ID,
            position.bytes.clone(),
            lamports,
        );
    }
    add_account_with_lamports(
        &mut test,
        reserve_admission,
        CLAIMS_PROGRAM_ID,
        admission
            .to_state_bytes()
            .expect("admission bytes")
            .to_vec(),
        admission_lamports,
    );
    add_account_with_lamports(
        &mut test,
        rent_credit,
        RENT_PROGRAM_ID,
        rent_credit_bytes,
        Rent::default().minimum_balance(128),
    );

    add_account(&mut test, root, CALLER_PROGRAM_ID, root_bytes);
    add_account(
        &mut test,
        shard_mint,
        token_program_id(),
        // A COORDINATE THE MARKET RESOLVED AWAY FROM HAS NO SHARDS LEFT, and
        // that is the lifecycle rather than a convenience. Holders of a losing
        // coordinate burn through `TerminalZeroBurn` (the atomic campaign
        // drives it), so by the time anybody compacts the reserve the supply is
        // gone. The wire refuses a zero RATE outright -- "a rate of zero
        // promises a record nobody would ever redeem" -- so zero *supply* is
        // the only way a fractional compaction ever reaches the plan's
        // no-claim branch, and modelling it any other way would be testing a
        // state the protocol cannot be in.
        mint_bytes(
            root,
            if winner == basis_profile.outcome() {
                OUTSTANDING_SHARDS
            } else {
                0
            },
            MINT_DECIMALS,
        ),
    );
    add_account(
        &mut test,
        HOLDER_SHARD_TOKENS,
        token_program_id(),
        token_account_bytes_for(
            shard_mint,
            holder,
            if winner == basis_profile.outcome() {
                OUTSTANDING_SHARDS
            } else {
                0
            },
        ),
    );
    let certificate_market = match plant {
        CampaignPlantV1::TerminalCertificateIdentityMismatch => Pubkey::new_from_array([0x85; 32]),
        _ => core_market,
    };
    add_account(
        &mut test,
        CERTIFICATE_ACCOUNT,
        CLAIMS_PROGRAM_ID,
        terminal_certificate_bytes(
            basis_profile,
            winner,
            certificate_market,
            shared.product.digest,
        ),
    );

    let custody_authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            core_market.as_ref(),
            release_set.as_slice(),
        ],
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            core_market.to_bytes(),
            release_set,
            CUSTODY_CONTEXT,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let custody_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            core_market.to_bytes(),
            release_set,
            CallerRoleV1::Claims,
            CUSTODY_CONTEXT,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    add_account(
        &mut test,
        COLLATERAL_MINT,
        token_program_id(),
        collateral_mint_bytes(HOARD_ATOMS, COLLATERAL_DECIMALS),
    );
    add_account(
        &mut test,
        hoard,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, custody_authority, HOARD_ATOMS),
    );
    add_account(
        &mut test,
        HOLDER_COLLATERAL_TOKENS,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, holder, 0),
    );
    add_account(
        &mut test,
        CRANKER_COLLATERAL_TOKENS,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, cranker, 0),
    );
    test.add_account(
        custody_replay,
        Account {
            lamports: Rent::default().minimum_balance(CUSTODY_REPLAY_BYTES_V1) * 2,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    for funded in [holder, opener, cranker] {
        test.add_account(
            funded,
            Account {
                lamports: 1_000_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }

    let escrow = Pubkey::find_program_address(
        &ClaimCheckEscrowSeedsV1::new(shared.claims_market.to_bytes())
            .expect("escrow seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let vault = Pubkey::find_program_address(
        &ClaimCheckVaultSeedsV1::new(shared.claims_market.to_bytes())
            .expect("vault seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;

    (
        test,
        CampaignFixture {
            plant,
            shared,
            release_set,
            activation_cache: activation_cache_key,
            realm_record,
            terms_record,
            behavior_record,
            root,
            shard_mint,
            holder_shard_tokens: HOLDER_SHARD_TOKENS,
            holder_collateral_tokens: HOLDER_COLLATERAL_TOKENS,
            cranker_collateral_tokens: CRANKER_COLLATERAL_TOKENS,
            holder,
            opener,
            cranker,
            custody_replay,
            custody_authority,
            hoard,
            rent_credit,
            reserve_admission,
            escrow,
            vault,
            winner,
            basis_profile,
            outcome: basis_profile.outcome(),
        },
    )
}

fn curved_price_gate_certificate(profile: CampaignBasisV1) -> [u8; PRICE_GATE_REQUEST_BYTES_V1] {
    let degree = profile.degree().expect("curved degree");
    let payouts: &[u64] = match profile {
        CampaignBasisV1::Categorical => panic!("categorical basis is price-gate exempt"),
        CampaignBasisV1::CurvedDegreeTwo => &[1, 4, 2],
        CampaignBasisV1::CurvedDegreeThree => &[1, 4, 4, 2],
    };
    let mut certificate = [0_u8; PRICE_GATE_REQUEST_BYTES_V1];
    certificate[PRICE_GATE_MAGIC_OFFSET_V1..PRICE_GATE_MAGIC_OFFSET_V1 + 8]
        .copy_from_slice(&PRICE_GATE_MAGIC_V1);
    certificate[PRICE_GATE_VERSION_OFFSET_V1..PRICE_GATE_VERSION_OFFSET_V1 + 2]
        .copy_from_slice(&PRICE_GATE_SCHEMA_VERSION_V1.to_le_bytes());
    certificate[PRICE_GATE_PROFILE_OFFSET_V1..PRICE_GATE_PROFILE_OFFSET_V1 + 2]
        .copy_from_slice(&PRICE_GATE_PROFILE_V1.to_le_bytes());
    certificate[PRICE_GATE_SCALE_OFFSET_V1..PRICE_GATE_SCALE_OFFSET_V1 + 4].copy_from_slice(
        &u32::try_from(profile.payout_scale())
            .expect("price-gate scale")
            .to_le_bytes(),
    );
    certificate[PRICE_GATE_MASS_OFFSET_V1..PRICE_GATE_MASS_OFFSET_V1 + 8]
        .copy_from_slice(&1_u64.to_le_bytes());
    certificate[PRICE_GATE_DEGREE_OFFSET_V1] = degree;
    certificate[PRICE_GATE_WIDTH_OFFSET_V1] =
        u8::try_from(profile.width()).expect("price-gate width");
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
                .expect("price-gate coordinate")
                .to_le_bytes(),
        );
    certificate[PRICE_GATE_DENOMINATORS_OFFSET_V1..PRICE_GATE_DENOMINATORS_OFFSET_V1 + 4]
        .copy_from_slice(
            &u32::try_from(CURVED_RESULT_DENOMINATOR)
                .expect("price-gate denominator")
                .to_le_bytes(),
        );
    certificate
}

// ---------------------------------------------------------------------------
// Driving
// ---------------------------------------------------------------------------

async fn account_at(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context.banks_client.get_account(key).await.expect("query")
}

fn account_image(address: Pubkey, account: Account) -> AccountImageV1 {
    AccountImageV1 {
        address: address.to_bytes(),
        owner: account.owner.to_bytes(),
        lamports: account.lamports,
        rent_epoch: account.rent_epoch,
        executable: account.executable,
        data_base64: STANDARD.encode(account.data),
    }
}

async fn emit_propagated_compaction_bridge(
    context: &mut ProgramTestContext,
    fixture: &CampaignFixture,
    payout_per_claim: u64,
) {
    let Some(preterminal) = propagated_preterminal() else {
        return;
    };
    let raw = env::var_os("DCLUTCH_CUBIC_COMPACTION_BRIDGE_OUT")
        .expect("unified compaction requires an output bridge");
    let path = PathBuf::from(raw);
    assert!(
        path.is_absolute(),
        "compaction bridge path must be absolute"
    );
    let image = |address, account| account_image(address, account);
    let record = record_address(fixture);
    let value = CompactionBridgeV1 {
        schema: COMPACTION_SCHEMA_V1.into(),
        preterminal_bridge_sha256: bridge_digest(
            &canonical_bytes(&preterminal).expect("canonical preterminal bridge"),
        ),
        preterminal,
        payout_per_claim,
        escrowed_collateral_atoms: RESERVE_NATIVE_CLAIMS * payout_per_claim,
        holder_collateral_token: fixture.holder_collateral_tokens.to_bytes(),
        closer_collateral_token: fixture.cranker_collateral_tokens.to_bytes(),
        closer: fixture.cranker.to_bytes(),
        record: image(
            record,
            account_at(context, record)
                .await
                .expect("claim-check record"),
        ),
        escrow: image(
            fixture.escrow,
            account_at(context, fixture.escrow).await.expect("escrow"),
        ),
        vault: image(
            fixture.vault,
            account_at(context, fixture.vault).await.expect("vault"),
        ),
        shard_mint: image(
            fixture.shard_mint,
            account_at(context, fixture.shard_mint)
                .await
                .expect("shard Mint"),
        ),
        collateral_mint: image(
            COLLATERAL_MINT,
            account_at(context, COLLATERAL_MINT)
                .await
                .expect("collateral Mint"),
        ),
        holder_shards: image(
            fixture.holder_shard_tokens,
            account_at(context, fixture.holder_shard_tokens)
                .await
                .expect("holder shards"),
        ),
        holder_collateral: image(
            fixture.holder_collateral_tokens,
            account_at(context, fixture.holder_collateral_tokens)
                .await
                .expect("holder collateral"),
        ),
        closer_collateral: image(
            fixture.cranker_collateral_tokens,
            account_at(context, fixture.cranker_collateral_tokens)
                .await
                .expect("closer collateral"),
        ),
    };
    value.validate().expect("compaction bridge contract");
    write_atomic(&path, &value).expect("atomic compaction bridge write");
}

async fn lamports_at(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    account_at(context, key).await.map_or(0, |a| a.lamports)
}

struct Outcome {
    accepted: bool,
    units: u64,
    result: Result<(), solana_sdk::transaction::TransactionError>,
}

fn custom_refusal(result: &Result<(), solana_sdk::transaction::TransactionError>) -> Option<u32> {
    match result {
        Err(solana_sdk::transaction::TransactionError::InstructionError(
            _,
            solana_sdk::instruction::InstructionError::Custom(code),
        )) => Some(*code),
        _ => None,
    }
}

/// Submit one instruction signed by the given party and the payer.
///
/// The holder's keypair is never passed here by any caller, and that absence is
/// the campaign's whole claim: nothing in this file can be explained by the
/// sleeping holder having authorised it, because the holder never signs.
async fn submit_as(
    context: &mut ProgramTestContext,
    label: &str,
    instruction: Instruction,
    signer: &Keypair,
) -> Outcome {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let payer = context.payer.insecure_clone();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, signer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .map(ToString::to_string)
        .expect("a submitted transaction carries its own signature");
    let wire_bytes = 1_usize
        + transaction.signatures.len() * 64
        + transaction.message_data().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("transaction processing");
    let units = processed
        .metadata
        .clone()
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    let logs = processed
        .metadata
        .clone()
        .map(|metadata| metadata.log_messages)
        .unwrap_or_default();
    let failure = processed.result.clone().err().map(|e| format!("{e:?}"));
    // A no-op unless the gauntlet asked for evidence, so this stays an ordinary
    // test and stops being invisible to the census the moment a runner sets
    // DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR.
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
    Outcome {
        accepted: processed.result.is_ok(),
        units,
        result: processed.result,
    }
}

/// Submit one holder-facing act. Kept separate from `submit_as` so every
/// compaction call site continues to make the sleeping-holder absence obvious.
async fn submit_holder_act(
    context: &mut ProgramTestContext,
    label: &str,
    instruction: Instruction,
    signer: &Keypair,
) -> Outcome {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let payer = context.payer.insecure_clone();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, signer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .map(ToString::to_string)
        .expect("a submitted transaction carries its own signature");
    let wire_bytes = 1_usize
        + transaction.signatures.len() * 64
        + transaction.message_data().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("transaction processing");
    let units = processed
        .metadata
        .clone()
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    let logs = processed
        .metadata
        .clone()
        .map(|metadata| metadata.log_messages)
        .unwrap_or_default();
    let failure = processed.result.clone().err().map(|e| format!("{e:?}"));
    // A no-op unless the gauntlet asked for evidence, so this stays an ordinary
    // test and stops being invisible to the census the moment a runner sets
    // DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR.
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
    Outcome {
        accepted: processed.result.is_ok(),
        units,
        result: processed.result,
    }
}

fn fractional_redemption_instruction(
    fixture: &CampaignFixture,
    request: dclutch_claims_svm::fractional_claim_check_v1::FractionalRedeemClaimCheckRequestV1,
    holder: Pubkey,
    holder_collateral_tokens: Pubkey,
    holder_shard_tokens: Pubkey,
) -> Instruction {
    use dclutch_claims_svm::fractional_claim_check_v1::FractionalClaimCheckRedemptionRoleV1 as Role;
    let accounts = Role::frame()
        .iter()
        .map(|role| {
            let key = match role {
                Role::Holder => holder,
                Role::FractionalClaimCheckRecord => record_address(fixture),
                Role::Escrow => fixture.escrow,
                Role::Vault => fixture.vault,
                Role::HolderCollateralTokens => holder_collateral_tokens,
                Role::CollateralMint => COLLATERAL_MINT,
                Role::ShardMint => fixture.shard_mint,
                Role::HolderShardTokens => holder_shard_tokens,
                Role::TokenProgram => token_program_id(),
                Role::FractionalCapabilityRoot => {
                    panic!("the retired root is not in the redemption frame")
                }
            };
            let (signer, writable) = role.privileges();
            if writable {
                AccountMeta::new(key, signer)
            } else {
                AccountMeta::new_readonly(key, signer)
            }
        })
        .collect();
    Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts,
        data: request
            .to_bytes()
            .expect("redemption request bytes")
            .to_vec(),
    }
}

fn close_settled_escrow_instruction(fixture: &CampaignFixture) -> Instruction {
    use dclutch_claims_svm::claim_check_request_v1::CloseClaimCheckEscrowRequestV1;
    Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.cranker, true),
            AccountMeta::new(fixture.escrow, false),
            AccountMeta::new(fixture.vault, false),
            AccountMeta::new(fixture.cranker_collateral_tokens, false),
            AccountMeta::new_readonly(COLLATERAL_MINT, false),
            AccountMeta::new_readonly(token_program_id(), false),
        ],
        data: CloseClaimCheckEscrowRequestV1 {
            aggregate: fixture.shared.claims_market.to_bytes(),
        }
        .new()
        .expect("escrow close request")
        .to_bytes()
        .expect("escrow close bytes")
        .to_vec(),
    }
}

async fn fractional_redemption_state(
    context: &mut ProgramTestContext,
    fixture: &CampaignFixture,
) -> Vec<Option<Account>> {
    let mut state = Vec::new();
    for key in [
        record_address(fixture),
        fixture.escrow,
        fixture.vault,
        fixture.shard_mint,
        fixture.holder_shard_tokens,
        fixture.holder_collateral_tokens,
    ] {
        state.push(account_at(context, key).await);
    }
    state
}

/// The stranger's permissionless escrow open: the deadline's origin.
fn open_instruction(fixture: &CampaignFixture) -> Instruction {
    let request = OpenClaimCheckEscrowRequestV1 {
        release_set: fixture.release_set,
        market: fixture.shared.core_market.to_bytes(),
        realm: fixture.realm_record.digest,
        generation: GENERATION,
    }
    .new()
    .expect("canonical open request");
    Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.opener, true),
            AccountMeta::new(fixture.escrow, false),
            AccountMeta::new(fixture.vault, false),
            AccountMeta::new_readonly(fixture.shared.claims_market, false),
            AccountMeta::new_readonly(fixture.shared.core_market, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.realm_record.raw, false),
            AccountMeta::new_readonly(fixture.realm_record.staging, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(COLLATERAL_MINT, false),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: request.to_bytes().expect("open request bytes").to_vec(),
    }
}

// ---------------------------------------------------------------------------
// The campaign
// ---------------------------------------------------------------------------

/// **Leg one: a stranger opens the escrow, and the clock starts.**
///
/// Permissionless by design and permissionless in fact: the opener is a key
/// with no relationship to the market, the holder or the reserve, and it is the
/// only signature on the transaction. What the open buys is the compaction
/// deadline's origin -- stamped here rather than at the market's terminal
/// transition, which can only ever lengthen a holder's grace period.
#[tokio::test]
async fn a_stranger_opens_the_escrow_and_pays_for_it() {
    let (test, fixture) = campaign_fixture();
    let mut context = test.start_with_context().await;

    assert!(
        account_at(&mut context, fixture.escrow).await.is_none()
            || account_at(&mut context, fixture.escrow)
                .await
                .expect("escrow")
                .data
                .is_empty(),
        "the escrow must not exist before the open"
    );
    let before = lamports_at(&mut context, fixture.opener).await;

    let outcome = submit_as(&mut context,
        "fractional claim-check: a stranger opens the escrow and pays for it", open_instruction(&fixture), &opener_keypair()).await;
    assert!(
        outcome.accepted,
        "the permissionless open must land: {:?} (code {:?})",
        outcome.result,
        custom_refusal(&outcome.result)
    );

    let escrow_account = account_at(&mut context, fixture.escrow)
        .await
        .expect("the escrow exists after the open");
    let escrow = ClaimCheckEscrowV1::decode(&escrow_account.data).expect("escrow decodes");
    assert_eq!(escrow.aggregate, fixture.shared.claims_market.to_bytes());
    assert_eq!(escrow.vault, fixture.vault.to_bytes());
    assert_eq!(escrow.collateral_mint, COLLATERAL_MINT.to_bytes());
    assert_eq!(escrow.opener, fixture.opener.to_bytes());
    assert_eq!(escrow.outstanding_claim_checks, 0);
    assert_eq!(escrow.generation, GENERATION);

    // THE OPENER IS OUT OF POCKET, and the record says by exactly how much.
    // This is what makes the open a funded position rather than a favour: the
    // outlay is a debt the first crank repays before it pays the cranker.
    let after = lamports_at(&mut context, fixture.opener).await;
    assert!(
        after < before,
        "the opener advances the rent for both accounts"
    );
    assert!(
        escrow.opener_outlay > 0,
        "the outlay the cranks repay must be recorded"
    );
    let vault = account_at(&mut context, fixture.vault)
        .await
        .expect("the vault exists");
    assert_eq!(token_amount(&vault.data), 0, "the vault opens empty");

    println!(
        "OPEN: opener_outlay={} opened_slot={} units={}",
        escrow.opener_outlay, escrow.opened_slot, outcome.units
    );
}

/// The deadline is a real wait, and the campaign warps rather than pretends.
#[test]
fn the_compaction_deadline_is_the_one_hundred_and_eighty_day_wait_the_design_states() {
    // 180 days at 400ms slots. Pinned as a literal beside the arithmetic so a
    // change to either has to argue with the other.
    assert_eq!(COMPACTION_DEADLINE_SLOTS_V1, 38_880_000);
    assert_eq!(
        COMPACTION_DEADLINE_SLOTS_V1,
        180 * 24 * 60 * 60 * 1_000 / 400
    );
}

// ---------------------------------------------------------------------------
// The crank
// ---------------------------------------------------------------------------

/// The exact terminal settlement a fractional compaction of this reserve is.
///
/// `caller_role` is `Claims` and the recipient pair is the ESCROW and its
/// VAULT, which is the whole economic difference from the holder's own
/// redemption: the same derivation, the same quantity, the same coordinate,
/// paying into an account only the holder can later open rather than into the
/// holder's wallet. That is what "compaction pays what timely redemption pays"
/// means, and it is a property of this request rather than of an assertion.
fn crank_terminal_request(
    fixture: &CampaignFixture,
    market_revision: u64,
) -> dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementRequestV3 {
    use dclutch_claims_svm::terminal_settlement_v3::{
        TerminalSettlementRequestInputV3, TerminalSettlementRequestV3,
    };
    let shared = &fixture.shared;
    TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
        caller_role: CallerRole::Claims,
        release_set: fixture.release_set,
        market: shared.core_market.to_bytes(),
        realm: fixture.realm_record.digest,
        // Free on this entry: the `parent_context == outer_request_digest` weld
        // belongs to the Trading entry, and a crank has no enclosing Trading
        // request to bind to. Nonzero because the wire refuses every unset
        // identity.
        parent_context: [0x9c; 32],
        product_record_digest: shared.product.digest,
        exposure_id: EXPOSURE_ID,
        exposure_digest: shared.exposure.digest,
        terminal_record_digest: CERTIFICATE_ACCOUNT.to_bytes(),
        owner: fixture.root.to_bytes(),
        position: shared.reserve_position.account.to_bytes(),
        recipient_owner: fixture.escrow.to_bytes(),
        recipient_token_account: fixture.vault.to_bytes(),
        claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
        custody_program: CUSTODY_PROGRAM_ID.to_bytes(),
        collateral_mint: COLLATERAL_MINT.to_bytes(),
        token_program: TOKEN_2022_PROGRAM_ID,
        semantic_basis_id: shared.semantic_basis_id,
        linked_basis_record_digest: shared.linked_basis.digest,
        generation: GENERATION,
        expected_market_revision: market_revision,
        expected_position_revision: POSITION_REVISION,
        expected_custody_revision: 1,
        quantity: RESERVE_NATIVE_CLAIMS,
        claim_index: fixture.outcome,
        transfer_index: 0,
    })
    .expect("crank terminal settlement request")
}

/// What the chain will pay, computed here first, bit for bit.
///
/// The Custody caller-authority PDA the frame must carry at coordinate 23 is
/// derived from the whole `CustodyRequestV1`, which commits to the candidate
/// digest, which commits to the payout and the signed-delta packet. So the only
/// way to NAME that account is to evaluate the terminal settlement host-side
/// exactly as Claims will, against the same authenticated bytes. Getting it
/// wrong does not produce a wrong number -- it produces an address the route
/// refuses, which is why this reproduction is a proof rather than a convenience.
fn crank_payout_and_caller(
    fixture: &CampaignFixture,
    request: dclutch_claims_svm::terminal_settlement_v3::TerminalSettlementRequestV3,
    market_revision: u64,
) -> (u64, Pubkey) {
    use dclutch_claims_svm::{
        product_basis_terminal_v3::{
            ProductClaimsTerminalAdmissionV3, ProductClaimsTerminalInputV3,
            encode_product_claims_terminal_signed_delta_v3,
        },
        signed_delta_v3::{DeltaDirectionV3, SignedDeltaV3},
        terminal_settlement_v3::TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
    };
    use dclutch_custody_contract::{ContextV1, CustodyRequestV1, OperationV1};
    use dclutch_fractional_atomic_program_test::narrow_fixture::{
        COORDINATE_DOMAIN_ID, EVALUATOR_RELEASE_ID, RESULT_UNIT_ID,
    };
    use dclutch_rational_representation_v2_kernel::product_v3::TerminalScenarioV3;
    use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
    use dclutch_representation_composition_v3_kernel::RecordAdmissionV3;
    use solana_program::hash::hashv;

    let shared = &fixture.shared;
    let position = &shared.reserve_position;
    let terminal_bytes = request.to_bytes();
    let terminal_digest = hash(&terminal_bytes).to_bytes();

    let admission = ProductClaimsTerminalAdmissionV3::new(
        EXPOSURE_ID,
        shared.exposure.digest,
        shared.product_id,
        shared.result_domain.digest,
        COORDINATE_DOMAIN_ID,
        RESULT_UNIT_ID,
        shared.semantic_basis_id,
        shared.linked_basis.digest,
        shared.core_market.to_bytes(),
        fixture.release_set,
        EVALUATOR_RELEASE_ID,
        shared.outcome_count,
        shared.payout_scale,
    )
    .expect("terminal admission");
    let width = shared.outcome_count as usize;
    let mut product_scratch = vec![0_u64; width];
    let mut translation_scratch = vec![0_u64; width];
    let mut claims_scratch = vec![0_u64; width];
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("neutral");
    let mut aggregate_scratch = vec![neutral; width];
    let mut packet = vec![
        0_u8;
        dclutch_claims_svm::signed_delta_v3::plan_bytes(
            dclutch_claims_svm::liability_basis_state_v2::LiabilityBasisMarketViewV2::decode(
                &shared.claims_market_bytes
            )
            .expect("aggregate decode")
            .claim_count,
            1,
            1
        )
        .expect("packet width")
    ];
    let payout = encode_product_claims_terminal_signed_delta_v3(
        ProductClaimsTerminalInputV3 {
            product_basis_bytes: &shared.linked_basis.bytes,
            admission,
            composition_exposure_bytes: &shared.exposure.bytes,
            composition_exposure_admission: RecordAdmissionV3 {
                selected_id: EXPOSURE_ID,
                finalized_id: EXPOSURE_ID,
                recomputed_digest: shared.exposure.digest,
                finalized_digest: shared.exposure.digest,
                record_authenticated: true,
            },
            product_record_digest: shared.product.digest,
            market_account: shared.claims_market.to_bytes(),
            market_bytes: &shared.claims_market_bytes,
            position_bytes: &position.bytes,
            owner: fixture.root.to_bytes(),
            request_id: terminal_digest,
            caller_role: CallerRole::Claims,
            // THE WINNER, from the fixture rather than from this coordinate.
            // It was hardcoded to `OUTCOME` until the zero-payout witness
            // caught it: on a market that resolved elsewhere the host-side
            // reproduction went on computing a paying scenario while the chain
            // computed a worthless one, and the two would have disagreed
            // silently. The chain derives this from the certificate; the
            // campaign must derive it from the same fact.
            terminal: match fixture.basis_profile {
                CampaignBasisV1::Categorical => TerminalScenarioV3::Categorical(fixture.winner),
                CampaignBasisV1::CurvedDegreeTwo | CampaignBasisV1::CurvedDegreeThree => {
                    TerminalScenarioV3::Rational {
                        numerator: CURVED_RESULT_NUMERATOR,
                        denominator: CURVED_RESULT_DENOMINATOR,
                    }
                }
            },
            claim_index: fixture.outcome,
            quantity: RESERVE_NATIVE_CLAIMS,
            expected_generation: GENERATION,
            expected_market_revision: market_revision,
            expected_position_revision: POSITION_REVISION,
            hoard_before: HOARD_ATOMS,
        },
        &mut product_scratch,
        &mut translation_scratch,
        &mut claims_scratch,
        &mut aggregate_scratch,
        &mut packet,
    )
    .expect("host-side terminal evaluation must agree with the chain");
    // Welded to the scenario rather than assumed. This was `payout > 0`, which
    // was true of every campaign that existed when it was written and became a
    // false assumption the moment one resolved elsewhere.
    match fixture.basis_profile {
        CampaignBasisV1::Categorical => assert_eq!(
            payout > 0,
            fixture.winner == fixture.outcome,
            "the reserve's coordinate pays exactly when the market resolved to it"
        ),
        CampaignBasisV1::CurvedDegreeTwo | CampaignBasisV1::CurvedDegreeThree => assert_eq!(
            payout,
            RESERVE_NATIVE_CLAIMS * 4,
            "the curved midpoint's cumulative-floor middle coordinate pays four atoms"
        ),
    }

    let candidate_digest = hashv(&[
        TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
        &terminal_digest,
        &hash(&packet).to_bytes(),
        &payout.to_le_bytes(),
        &shared.exposure.digest,
        &CERTIFICATE_ACCOUNT.to_bytes(),
    ])
    .to_bytes();
    // A ZERO SETTLEMENT SENDS NO CUSTODY REQUEST, so there is no request digest
    // to derive a caller authority from -- `CustodyRequestV1::to_bytes` refuses
    // a zero-amount transfer outright as `InvalidOperationShape`, which is the
    // contract saying the same thing. That is exactly why
    // `authenticate_zero_custody_accounts` requires coordinate 23 to be
    // literally the Claims program: there is no authority to put there.
    // Returned from here rather than branched at the frame builder, so the fact
    // that decides the account has one author.
    if payout == 0 {
        return (0, CLAIMS_PROGRAM_ID);
    }
    let custody_request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: fixture.release_set,
        market: shared.core_market.to_bytes(),
        realm: fixture.realm_record.digest,
        context: CUSTODY_CONTEXT,
        caller_program: CLAIMS_PROGRAM_ID.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: [0; 32],
            // THE ESCROW, not the holder. The one substitution that makes this
            // a compaction, and it is inside the digest the caller authority is
            // derived from -- so a campaign that named the holder here would
            // derive an address the route refuses rather than a payout that
            // went to the wrong place.
            destination_owner: fixture.escrow.to_bytes(),
            order: [0; 32],
            parent_request_digest: terminal_digest,
            order_nonce: POSITION_REVISION,
            generation: GENERATION,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: fixture.hoard.to_bytes(),
        destination: fixture.vault.to_bytes(),
        source_vault_context: CUSTODY_CONTEXT,
        destination_vault_context: [0; 32],
        mint: COLLATERAL_MINT.to_bytes(),
        token_program: TOKEN_2022_PROGRAM_ID,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: 1,
        resulting_revision: 2,
        amount: payout,
        rent_lamports: 0,
    };
    let custody_bytes = custody_request.to_bytes().expect("custody request bytes");
    let caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            fixture.release_set,
            shared.core_market.to_bytes(),
            ExecutionRoleV1::Claims,
            CUSTODY_CONTEXT,
            hash(&custody_bytes).to_bytes(),
        )
        .expect("claims custody caller seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    (payout, caller)
}

/// The wrapped 36-account terminal frame, in contract coordinate order.
///
/// The sibling terminal campaign's first thirty-six accounts, with exactly three
/// substitutions, and each one is what makes this a compaction rather than a
/// redemption:
///
/// - **coordinate 0 is the CRANKER**, signing, where a Trading-composed
///   settlement carries a release-scoped caller authority. Under
///   `(Claims, ClaimCheckCrank)` coordinate 0 is asked only that somebody stood
///   behind the transaction -- the deliberate relaxation that makes the crank
///   permissionless, and the reason a party with no authority can be here.
/// - **coordinate 23 is the crank's own Custody caller authority**, derived
///   above from the payout this campaign reproduced.
/// - **coordinate 33 is the escrow's VAULT**, where a redemption carries the
///   holder's own token account. This is the collateral's destination, and it
///   is the single substitution that turns a payout the holder cannot collect
///   into one they can collect whenever they return.
fn terminal_prefix(fixture: &CampaignFixture, custody_caller: Pubkey) -> Vec<AccountMeta> {
    let shared = &fixture.shared;
    let (caller_program, caller_programdata) = match fixture.plant {
        CampaignPlantV1::SignedDeltaCallerReleaseMismatch => {
            (CALLER_PROGRAM_ID, programdata_address(CALLER_PROGRAM_ID))
        }
        _ => (CLAIMS_PROGRAM_ID, programdata_address(CLAIMS_PROGRAM_ID)),
    };

    let accounts = vec![
        // WRITABLE, and this is the coordinate that pays the cranker. The
        // sibling terminal frame carries a read-only caller authority here; a
        // compaction carries the party the sweep rewards, so a read-only meta
        // is refused by the runtime itself as `ReadonlyLamportChange` the
        // moment `close_and_split` credits it.
        AccountMeta::new(fixture.cranker, true),
        AccountMeta::new(shared.claims_market, false),
        AccountMeta::new_readonly(shared.linked_basis.raw, false),
        AccountMeta::new_readonly(shared.linked_basis.staging, false),
        AccountMeta::new_readonly(shared.product.raw, false),
        AccountMeta::new_readonly(shared.product.staging, false),
        AccountMeta::new_readonly(shared.result_domain.raw, false),
        AccountMeta::new_readonly(shared.result_domain.staging, false),
        AccountMeta::new_readonly(shared.portfolio.raw, false),
        AccountMeta::new_readonly(shared.portfolio.staging, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(shared.core_market, false),
        AccountMeta::new_readonly(fixture.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        // COORDINATE 14 IS THE CALLER PROGRAM, AND THE CALLER HERE IS CLAIMS.
        // The sibling terminal campaign puts its test caller here because its
        // request is `CallerRole::Trading`; a compaction pins
        // `CallerRole::Claims`, which names the case with no caller program at
        // all, and the release authentication resolves this coordinate against
        // the activation cache's binding for whichever role the request states.
        // Putting the test signer here -- the obvious copy from the sibling --
        // is refused as `SignedDeltaSbfErrorV3::Release`, and it took reading
        // that refusal to find it.
        AccountMeta::new_readonly(caller_program, false),
        AccountMeta::new_readonly(caller_programdata, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CORE_PROGRAM_ID), false),
        AccountMeta::new(shared.reserve_position.account, false),
        AccountMeta::new_readonly(shared.exposure.raw, false),
        AccountMeta::new_readonly(shared.exposure.staging, false),
        AccountMeta::new_readonly(custody_caller, false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(CERTIFICATE_ACCOUNT, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
        AccountMeta::new_readonly(fixture.realm_record.raw, false),
        AccountMeta::new_readonly(fixture.realm_record.staging, false),
        AccountMeta::new(fixture.custody_replay, false),
        AccountMeta::new_readonly(COLLATERAL_MINT, false),
        AccountMeta::new(fixture.hoard, false),
        AccountMeta::new(fixture.vault, false),
        AccountMeta::new_readonly(fixture.custody_authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    assert_eq!(
        accounts.len(),
        dclutch_claims_svm::terminal_settlement_v3::TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3
    );
    accounts
}

/// The fourteen accounts compaction adds, BUILT FROM THE DECLARATION.
///
/// The address for each role is chosen here; the privileges never are. They come
/// from `FractionalCompactionRoleV1::privileges`, the same function the route's
/// own frame guard reads, so this campaign cannot present a frame the
/// declaration does not describe -- and cannot quietly keep passing if the
/// declaration changes under it. Writing `AccountMeta::new` or `new_readonly`
/// by hand here would have made the campaign a second opinion about the frame
/// instead of a driver of it.
fn compaction_accounts(fixture: &CampaignFixture, record: Pubkey) -> Vec<AccountMeta> {
    use dclutch_claims_svm::fractional_claim_check_v1::FractionalCompactionRoleV1 as Role;
    Role::frame()
        .into_iter()
        .map(|role| {
            let key = match role {
                Role::Escrow => fixture.escrow,
                Role::FractionalClaimCheckRecord => record,
                Role::ReserveAdmission => fixture.reserve_admission,
                Role::RentCredit => fixture.rent_credit,
                Role::Opener => fixture.opener,
                Role::SystemProgram => system_program::ID,
                Role::FractionalCapabilityRoot => fixture.root,
                Role::ShardMint => fixture.shard_mint,
                Role::ShardTokenProgram => token_program_id(),
                Role::ExposureTerms => fixture.terms_record.raw,
                Role::ExposureTermsStaging => fixture.terms_record.staging,
                Role::TokenBehavior => fixture.behavior_record.raw,
                Role::TokenBehaviorStaging => fixture.behavior_record.staging,
                Role::RentProgram => RENT_PROGRAM_ID,
                other => panic!("{other:?} is refused and must never be in a built frame"),
            };
            let (signer, writable) = role.privileges();
            // THE ONE PLACE THE OUTER FRAME DIFFERS FROM THE DECLARATION, and
            // it is not a discrepancy -- it is what "Trading-composed" means.
            // The declaration describes the frame the CLAIMS ROUTE sees, and
            // there the capability root is a signer. It becomes one inside the
            // caller's `invoke_signed`; the root is a program-derived address
            // and this transaction has no key that could sign for it, so
            // presenting it as a signer here is not a stricter frame but an
            // unsignable one. Cleared explicitly rather than by writing the
            // metas out by hand, so every other privilege still comes from the
            // declaration and a change there still reaches this campaign.
            let outer_signer = signer && !matches!(role, Role::FractionalCapabilityRoot);
            AccountMeta {
                pubkey: key,
                is_signer: outer_signer,
                is_writable: writable,
            }
        })
        .collect()
}

/// The fractional claim-check this crank mints, at its derived address.
fn record_address(fixture: &CampaignFixture) -> Pubkey {
    use dclutch_claims_svm::fractional_claim_check_v1::FractionalClaimCheckSeedsV1;
    Pubkey::find_program_address(
        &FractionalClaimCheckSeedsV1::new(
            fixture.shared.claims_market.to_bytes(),
            fixture.shard_mint.to_bytes(),
        )
        .expect("record seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0
}

/// One compaction instruction: the caller program, then the exact 50-account frame.
fn compaction_instruction(
    fixture: &CampaignFixture,
    request: &dclutch_claims_svm::fractional_claim_check_compaction_request_v1::FractionalCompactToClaimCheckRequestV1,
    custody_caller: Pubkey,
    action: dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1,
) -> Instruction {
    use dclutch_claims_svm::fractional_claim_check_v1::FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1;
    let mut accounts = Vec::with_capacity(FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 + 1);
    accounts.push(AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false));
    accounts.extend(terminal_prefix(fixture, custody_caller));
    accounts.extend(compaction_accounts(fixture, record_address(fixture)));
    assert_eq!(accounts.len(), FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 + 1);

    let mut data = Vec::with_capacity(
        dclutch_fractional_compaction_test_caller_sbf::FRACTIONAL_COMPACTION_TEST_WRAPPER_BYTES,
    );
    data.push(action as u8);
    data.extend_from_slice(&request.to_bytes().expect("compaction request bytes"));
    Instruction {
        program_id: CALLER_PROGRAM_ID,
        accounts,
        data,
    }
}

fn programdata_address(program: Pubkey) -> Pubkey {
    dclutch_fractional_atomic_program_test::campaign_support::programdata_address(program)
}

/// **THE CAMPAIGN: a stranger compacts a sleeping holder's reserve, and the
/// conservation table is read off the transaction rather than off the plan.**
///
/// Every number printed below is observed on chain, before and after, by
/// querying accounts. None of it is the design's arithmetic restated: this
/// thread's five refusals were all protecting exactly that distinction, and a
/// table computed from the plan would be the plan agreeing with itself.
#[tokio::test]
async fn a_stranger_compacts_a_sleeping_holders_reserve_and_nothing_leaks() {
    use dclutch_claims_svm::{
        fractional_claim_check_compaction_request_v1::{
            FractionalCompactToClaimCheckRequestV1, FractionalCompactionCoordinatesV1,
        },
        liability_basis_state_v2::LiabilityBasisMarketViewV2,
    };
    use dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1;

    let (test, fixture) = campaign_fixture();
    let mut context = test.start_with_context().await;

    // --- the stranger opens the escrow, and the 180-day clock starts ---------
    let opened = submit_as(&mut context,
        "fractional claim-check: a stranger compacts a sleeping holders reserve and nothing leaks", open_instruction(&fixture), &opener_keypair()).await;
    assert!(opened.accepted, "the open must land: {:?}", opened.result);
    create_custody_replay(&mut context, &fixture).await;
    let escrow = ClaimCheckEscrowV1::decode(
        &account_at(&mut context, fixture.escrow)
            .await
            .expect("escrow")
            .data,
    )
    .expect("escrow decodes");

    // --- what the chain will pay, reproduced here first ----------------------
    let market_revision = LiabilityBasisMarketViewV2::decode(&fixture.shared.claims_market_bytes)
        .expect("aggregate decode")
        .revision;
    let terminal = crank_terminal_request(&fixture, market_revision);
    let (payout, custody_caller) = crank_payout_and_caller(&fixture, terminal, market_revision);

    // The rate the record will persist. Whole claims floor, exactly as
    // `divide_exposure_shards_v2` does, and the plan refuses unless the rate
    // times the claims equals the atoms the vault actually received.
    let whole_claims = OUTSTANDING_SHARDS / DENOMINATOR;
    assert_eq!(whole_claims, RESERVE_NATIVE_CLAIMS);
    assert_eq!(
        payout % whole_claims,
        0,
        "the fixture must pay a whole rate per claim, or the plan cannot balance"
    );
    let payout_per_claim = payout / whole_claims;

    let request = FractionalCompactToClaimCheckRequestV1::new(
        FractionalCompactionCoordinatesV1 {
            terms: fixture.terms_record.digest,
            token_behavior: fixture.behavior_record.digest,
            expected_root_revision: ROOT_REVISION,
            representation_coordinate: fixture.outcome,
            payout_per_claim,
        },
        crank_terminal_request(&fixture, market_revision),
    )
    .expect("canonical fractional compaction request");

    // --- the wait, warped rather than pretended ------------------------------
    context
        .warp_to_slot(escrow.opened_slot + COMPACTION_DEADLINE_SLOTS_V1)
        .expect("warp past the compaction deadline");

    // --- observe everything the crank can move, before it runs ---------------
    let record = record_address(&fixture);
    let position = fixture.shared.reserve_position.account;
    let before_hoard = token_amount(
        &account_at(&mut context, fixture.hoard)
            .await
            .expect("hoard")
            .data,
    );
    let before_vault = token_amount(
        &account_at(&mut context, fixture.vault)
            .await
            .expect("vault")
            .data,
    );
    let before_position = lamports_at(&mut context, position).await;
    let before_admission = lamports_at(&mut context, fixture.reserve_admission).await;
    let before_rent_credit = lamports_at(&mut context, fixture.rent_credit).await;
    let before_opener = lamports_at(&mut context, fixture.opener).await;
    let before_cranker = lamports_at(&mut context, fixture.cranker).await;
    let before_record = lamports_at(&mut context, record).await;
    let before_holder = lamports_at(&mut context, fixture.holder).await;

    // --- THE CRANK -----------------------------------------------------------
    let outcome = submit_as(
        &mut context,
        "fractional claim-check: a stranger compacts a sleeping holders reserve and nothing leaks [2]",
        compaction_instruction(
            &fixture,
            &request,
            custody_caller,
            FractionalCompactionCallerActionV1::Signed,
        ),
        &cranker_keypair(),
    )
    .await;
    assert!(
        outcome.accepted,
        "the compaction must land: {:?} (code {:?})",
        outcome.result,
        custom_refusal(&outcome.result)
    );

    // --- WITNESS w7, and it is an observation rather than an assertion -------
    //
    // Design §17.8 ruling 2 dropped `TradingCallerAuthority` on the argument
    // that it refused nothing. §17.9 recorded the ruling as "argued, not
    // driven", because the frame declaration saying an account is absent is not
    // the same as a route running without it. It has now run without it: the
    // compaction above SUCCEEDED, over a frame built from the declaration, in
    // which no coordinate holds a caller authority because the declaration
    // gives the role no index to sit at -- and the test signer that produced the
    // root's signature contains no code that could derive one.
    //
    // The three facts together are w7. Any one alone is weaker: the index being
    // `None` is a claim about the enum, the frame width is a claim about the
    // builder, and only the acceptance above is a claim about the route.
    use dclutch_claims_svm::fractional_claim_check_v1::{
        FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1, FractionalCompactionRoleV1,
    };
    assert_eq!(
        FractionalCompactionRoleV1::TradingCallerAuthority.index(),
        None
    );
    assert_eq!(FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1, 50);
    assert_eq!(
        compaction_accounts(&fixture, record).len()
            + terminal_prefix(&fixture, custody_caller).len(),
        FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1,
        "the frame the route just accepted is the whole declared frame and no more"
    );

    // --- observe again -------------------------------------------------------
    let after_hoard = token_amount(
        &account_at(&mut context, fixture.hoard)
            .await
            .expect("hoard")
            .data,
    );
    let after_vault = token_amount(
        &account_at(&mut context, fixture.vault)
            .await
            .expect("vault")
            .data,
    );
    let after_position = lamports_at(&mut context, position).await;
    let after_admission = lamports_at(&mut context, fixture.reserve_admission).await;
    let after_rent_credit = lamports_at(&mut context, fixture.rent_credit).await;
    let after_opener = lamports_at(&mut context, fixture.opener).await;
    let after_cranker = lamports_at(&mut context, fixture.cranker).await;
    let after_record = lamports_at(&mut context, record).await;
    let after_holder = lamports_at(&mut context, fixture.holder).await;

    // --- COLLATERAL: conserved to the atom -----------------------------------
    assert_eq!(
        before_hoard - after_hoard,
        after_vault - before_vault,
        "every atom that left the hoard must be in the vault; a difference is collateral \
         that went somewhere this campaign is not looking"
    );
    assert_eq!(
        after_vault - before_vault,
        payout,
        "the vault must receive exactly what the holder's own redemption would have paid"
    );
    assert_eq!(
        payout,
        whole_claims * payout_per_claim,
        "the escrowed atoms must equal the rate the record persists, or the last holder \
         back is unpayable"
    );

    // --- THE POSITION IS GONE, AND ITS ADMISSION WITH IT ---------------------
    assert_eq!(after_position, 0, "the reserve Position must be closed");
    assert_eq!(after_admission, 0, "its admission must be closed with it");

    // --- LAMPORTS: the sweep goes somewhere, and nowhere else ----------------
    let swept = before_position + before_admission;
    let to_rent_credit = after_rent_credit - before_rent_credit;
    let to_opener = after_opener - before_opener;
    let record_rent = after_record - before_record;
    // The cranker's delta is a CLEAN credit, and that is worth stating because
    // it is easy to get wrong: the transaction's fee payer is the harness payer,
    // not the cranker, so nothing subtracts from this balance. A campaign whose
    // cranker was also the fee payer would net the reward against the fee and
    // report a number that is neither.
    let to_cranker = after_cranker - before_cranker;
    assert_eq!(
        swept,
        record_rent + to_opener + to_cranker + to_rent_credit,
        "every lamport the closed accounts held must land in exactly one of the four \
         sinks: the record's rent, the opener's debt, the crank's reward, and the \
         residue. A shortfall is a lamport burned; an excess is one conjured."
    );

    // --- THE REMAINDER GOES NOWHERE ------------------------------------------
    //
    // Stated as an identity rather than as a positive residue, and the first
    // version of this assertion got it wrong: it required
    // `to_rent_credit > 0`, and the campaign reported zero. Zero is correct.
    // The amended order pays the record's rent, then the crank, then the
    // opener's debt, and the RentCredit takes what is LEFT -- which on this
    // market is nothing, because the two closed accounts did not hold more than
    // those three claims. "The remainder goes nowhere" is the absence of a
    // fifth term in the equation above, not a number that happens to be
    // positive; a campaign that demanded a positive residue would be demanding
    // a fixture rich enough to leave one, and would have called a correct
    // route wrong.
    assert!(
        to_rent_credit <= swept,
        "the residue cannot exceed what was swept"
    );

    // --- AND WHAT THE TABLE SAYS ABOUT THE OPENER, RECORDED RATHER THAN
    //     SMOOTHED OVER ---------------------------------------------------
    //
    // On this market one compaction does NOT make the opener whole: it advanced
    // `escrow.opener_outlay` and recovers `to_opener`, which is less. That is
    // the amended order working as designed rather than a defect -- the record's
    // rent is paid first because the claim-check must exist, the crank is paid
    // second because a crank nobody pays does not happen, and the opener takes
    // what is left of two accounts' rent. It is asserted rather than merely
    // printed so that a future change which silently starts over-repaying the
    // opener (out of the residue, or out of the record's rent) has to argue
    // with a line rather than slip through a table nobody reads.
    assert!(
        to_opener <= escrow.opener_outlay,
        "the sweep may repay the opener's debt and must never pay them MORE than \
         they advanced -- the escrow records an outlay, not an income"
    );
    let unrecovered = escrow.opener_outlay - to_opener;
    assert!(
        record_rent > 0 && to_cranker > 0,
        "the two claims that rank above the opener both took something, which is \
         why the opener is short; if either were zero the shortfall would need \
         another explanation"
    );

    // --- THE SLEEPING HOLDER IS UNTOUCHED ------------------------------------
    //
    // They signed nothing -- no caller in this file ever passes their keypair --
    // and now: they paid nothing either. Not one lamport of the rent the crank
    // recovers came from the party the crank is performed on behalf of, and
    // their wallet is exactly where they left it. That is the difference
    // between a permissionless crank and a fee levied on the absent, and it is
    // the one property a holder who never comes back would care about most.
    assert_eq!(
        after_holder, before_holder,
        "a compaction must cost the sleeping holder nothing at all"
    );

    // --- THE RECORD NAMES THE ESCROW, AND NO PAYEE ---------------------------
    let record_account = account_at(&mut context, record)
        .await
        .expect("the fractional claim-check exists");
    assert_eq!(record_account.owner, CLAIMS_PROGRAM_ID);
    assert!(!record_account.data.is_empty());

    println!("\n=== FRACTIONAL COMPACTION: CONSERVATION, FROM THE TRANSACTION ===");
    println!("compute units                 {}", outcome.units);
    println!("-- collateral (atoms) --");
    println!(
        "hoard      {:>12} -> {:>12}  ({:+})",
        before_hoard,
        after_hoard,
        after_hoard as i128 - before_hoard as i128
    );
    println!(
        "vault      {:>12} -> {:>12}  ({:+})",
        before_vault,
        after_vault,
        after_vault as i128 - before_vault as i128
    );
    println!("payout                        {payout}");
    println!("whole claims                  {whole_claims}");
    println!("payout per claim              {payout_per_claim}");
    println!("-- lamports --");
    println!(
        "position   {:>12} -> {:>12}",
        before_position, after_position
    );
    println!(
        "admission  {:>12} -> {:>12}",
        before_admission, after_admission
    );
    println!("swept                         {swept}");
    println!("  -> claim-check rent         {record_rent}");
    println!("  -> opener repaid            {to_opener}");
    println!("  -> cranker reward           {to_cranker}");
    println!("  -> rent credit (residue)    {to_rent_credit}");
    println!("  -> burned                   0");
    println!("opener outlay recorded        {}", escrow.opener_outlay);
    println!("opener still out of pocket    {unrecovered}");
    println!("================================================================\n");
}

/// Drive one curved ProductBasisV3 through the whole fractional holder life.
///
/// Both the quadratic and cubic choose a middle claim paying four atoms, so the
/// same physical lifecycle can compare equal economics while changing the
/// Product's exact evaluator and full partition.
async fn drive_curve_holder_life(basis_profile: CampaignBasisV1, expected_partition: &[u64]) {
    use dclutch_claims_svm::{
        fractional_claim_check_compaction_request_v1::{
            FractionalCompactToClaimCheckRequestV1, FractionalCompactionCoordinatesV1,
        },
        liability_basis_state_v2::LiabilityBasisMarketViewV2,
    };
    use dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1;

    let (test, fixture) = campaign_fixture_basis(
        CampaignPlantV1::Faithful,
        basis_profile,
        basis_profile.outcome(),
    );
    let basis = ProductBasisV3::decode(&fixture.shared.linked_basis.bytes)
        .expect("finalized curved ProductBasisV3");
    let mut partition = vec![0_u64; basis_profile.width()];
    basis
        .evaluate_rational(
            CURVED_RESULT_NUMERATOR,
            CURVED_RESULT_DENOMINATOR,
            &mut partition,
        )
        .expect("exact curved midpoint");
    assert_eq!(partition.as_slice(), expected_partition);
    assert_eq!(fixture.shared.payout_scale, basis_profile.payout_scale());

    let mut context = test.start_with_context().await;
    let opened = submit_as(&mut context,
        "fractional claim-check: drive curve holder life", open_instruction(&fixture), &opener_keypair()).await;
    assert!(
        opened.accepted,
        "the escrow open must land: {:?}",
        opened.result
    );
    create_custody_replay(&mut context, &fixture).await;
    let escrow = ClaimCheckEscrowV1::decode(
        &account_at(&mut context, fixture.escrow)
            .await
            .expect("escrow")
            .data,
    )
    .expect("escrow decodes");
    let market_revision = LiabilityBasisMarketViewV2::decode(&fixture.shared.claims_market_bytes)
        .expect("aggregate decode")
        .revision;
    let terminal = crank_terminal_request(&fixture, market_revision);
    let (payout, custody_caller) = crank_payout_and_caller(&fixture, terminal, market_revision);
    assert_eq!(
        payout,
        RESERVE_NATIVE_CLAIMS
            * partition[usize::try_from(fixture.outcome).expect("curved outcome")]
    );
    assert_eq!(payout, 16);

    let whole_claims = OUTSTANDING_SHARDS / DENOMINATOR;
    let payout_per_claim = payout / whole_claims;
    assert_eq!(whole_claims, RESERVE_NATIVE_CLAIMS);
    assert_eq!(payout_per_claim, 4);
    let request = FractionalCompactToClaimCheckRequestV1::new(
        FractionalCompactionCoordinatesV1 {
            terms: fixture.terms_record.digest,
            token_behavior: fixture.behavior_record.digest,
            expected_root_revision: ROOT_REVISION,
            representation_coordinate: fixture.outcome,
            payout_per_claim,
        },
        crank_terminal_request(&fixture, market_revision),
    )
    .expect("canonical curved compaction request");
    context
        .warp_to_slot(escrow.opened_slot + COMPACTION_DEADLINE_SLOTS_V1)
        .expect("warp past the real deadline");

    let record = record_address(&fixture);
    let position = fixture.shared.reserve_position.account;
    let before_hoard = token_amount(
        &account_at(&mut context, fixture.hoard)
            .await
            .expect("hoard")
            .data,
    );
    let before_vault = token_amount(
        &account_at(&mut context, fixture.vault)
            .await
            .expect("vault")
            .data,
    );
    let before_position = lamports_at(&mut context, position).await;
    let before_admission = lamports_at(&mut context, fixture.reserve_admission).await;
    let before_rent_credit = lamports_at(&mut context, fixture.rent_credit).await;
    let before_opener = lamports_at(&mut context, fixture.opener).await;
    let before_cranker = lamports_at(&mut context, fixture.cranker).await;
    let before_record = lamports_at(&mut context, record).await;
    let before_holder = lamports_at(&mut context, fixture.holder).await;

    let outcome = submit_as(
        &mut context,
        "fractional claim-check: drive curve holder life [2]",
        compaction_instruction(
            &fixture,
            &request,
            custody_caller,
            FractionalCompactionCallerActionV1::Signed,
        ),
        &cranker_keypair(),
    )
    .await;
    assert!(
        outcome.accepted,
        "the curved compaction must land: {:?} (code {:?})",
        outcome.result,
        custom_refusal(&outcome.result)
    );

    let after_hoard = token_amount(
        &account_at(&mut context, fixture.hoard)
            .await
            .expect("hoard")
            .data,
    );
    let after_vault = token_amount(
        &account_at(&mut context, fixture.vault)
            .await
            .expect("vault")
            .data,
    );
    assert_eq!(before_hoard - after_hoard, payout);
    assert_eq!(after_vault - before_vault, payout);
    assert_eq!(lamports_at(&mut context, position).await, 0);
    assert_eq!(
        lamports_at(&mut context, fixture.reserve_admission).await,
        0
    );
    assert_eq!(
        lamports_at(&mut context, fixture.holder).await,
        before_holder
    );

    let after_rent_credit = lamports_at(&mut context, fixture.rent_credit).await;
    let after_opener = lamports_at(&mut context, fixture.opener).await;
    let after_cranker = lamports_at(&mut context, fixture.cranker).await;
    let after_record = lamports_at(&mut context, record).await;
    assert_eq!(
        before_position + before_admission,
        (after_record - before_record)
            + (after_opener - before_opener)
            + (after_cranker - before_cranker)
            + (after_rent_credit - before_rent_credit),
        "the curved route strands or conjures no rent lamport"
    );
    let record_account = account_at(&mut context, record)
        .await
        .expect("curved fractional claim-check");
    assert_eq!(record_account.owner, CLAIMS_PROGRAM_ID);
    assert!(!record_account.data.is_empty());

    use dclutch_claims_svm::fractional_claim_check_v1::{
        FractionalClaimCheckV1, FractionalRedeemClaimCheckRequestV1,
    };
    let persisted =
        FractionalClaimCheckV1::decode(&record_account.data).expect("fractional record decodes");
    assert_eq!(persisted.escrowed_atoms, payout);
    assert_eq!(persisted.denominator, DENOMINATOR);
    assert_eq!(persisted.payout_per_claim, payout_per_claim);
    let escrow_after_compaction = ClaimCheckEscrowV1::decode(
        &account_at(&mut context, fixture.escrow)
            .await
            .expect("escrow")
            .data,
    )
    .expect("escrow decodes");
    assert_eq!(escrow_after_compaction.outstanding_claim_checks, 1);
    assert_eq!(
        token_amount(
            &account_at(&mut context, fixture.holder_shard_tokens)
                .await
                .expect("holder shards")
                .data
        ),
        OUTSTANDING_SHARDS
    );
    assert_eq!(
        mint_supply(
            &account_at(&mut context, fixture.shard_mint)
                .await
                .expect("shard mint")
                .data
        ),
        OUTSTANDING_SHARDS
    );

    if basis_profile == CampaignBasisV1::CurvedDegreeThree {
        emit_propagated_compaction_bridge(&mut context, &fixture, payout_per_claim).await;
    }

    let request = |requested_shard_atoms| {
        FractionalRedeemClaimCheckRequestV1 {
            aggregate: fixture.shared.claims_market.to_bytes(),
            shard_mint: fixture.shard_mint.to_bytes(),
            requested_shard_atoms,
        }
        .new()
        .expect("fractional redemption request")
    };

    // Every honest local refusal is atomic under the real ELF. Each one starts
    // and ends with byte-identical program and Token accounts.
    for (instruction, signer, expected_code, label) in [
        (
            fractional_redemption_instruction(
                &fixture,
                request(DENOMINATOR - 1),
                fixture.holder,
                fixture.holder_collateral_tokens,
                fixture.holder_shard_tokens,
            ),
            holder_keypair(),
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckRedemptionSbfErrorV1::NoWholeClaim as u32,
            "sub-denominator dust",
        ),
        (
            fractional_redemption_instruction(
                &fixture,
                request(OUTSTANDING_SHARDS + 1),
                fixture.holder,
                fixture.holder_collateral_tokens,
                fixture.holder_shard_tokens,
            ),
            holder_keypair(),
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckRedemptionSbfErrorV1::Conservation as u32,
            "holder overdraw",
        ),
        (
            fractional_redemption_instruction(
                &fixture,
                request(DENOMINATOR),
                fixture.holder,
                fixture.cranker_collateral_tokens,
                fixture.holder_shard_tokens,
            ),
            holder_keypair(),
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckRedemptionSbfErrorV1::Authority as u32,
            "substituted payout owner",
        ),
        (
            fractional_redemption_instruction(
                &fixture,
                request(DENOMINATOR),
                fixture.cranker,
                fixture.holder_collateral_tokens,
                fixture.holder_shard_tokens,
            ),
            cranker_keypair(),
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckRedemptionSbfErrorV1::Authority as u32,
            "substituted shard authority",
        ),
    ] {
        let before = fractional_redemption_state(&mut context, &fixture).await;
        let refused = submit_holder_act(&mut context,
        "fractional claim-check: drive curve holder life [3]", instruction, &signer).await;
        assert!(!refused.accepted, "{label} must refuse");
        assert_eq!(custom_refusal(&refused.result), Some(expected_code), "{label}");
        assert_eq!(
            fractional_redemption_state(&mut context, &fixture).await,
            before,
            "{label} changed state despite refusal"
        );
    }

    let before_premature_close = fractional_redemption_state(&mut context, &fixture).await;
    let premature_close = submit_holder_act(
        &mut context,
        "fractional claim-check: drive curve holder life [4]",
        close_settled_escrow_instruction(&fixture),
        &cranker_keypair(),
    )
    .await;
    assert!(!premature_close.accepted);
    assert_eq!(
        custom_refusal(&premature_close.result),
        Some(
            dclutch_claims_sbf::claim_check_redemption_v1::ClaimCheckRedemptionSbfErrorV1::Vault
                as u32
        )
    );
    assert_eq!(
        fractional_redemption_state(&mut context, &fixture).await,
        before_premature_close,
        "a live fractional record must make escrow close roll back"
    );

    let record_rent = record_account.lamports;
    let holder_lamports_before_partial = lamports_at(&mut context, fixture.holder).await;
    let partial = submit_holder_act(
        &mut context,
        "fractional claim-check: drive curve holder life [5]",
        fractional_redemption_instruction(
            &fixture,
            request(2 * DENOMINATOR),
            fixture.holder,
            fixture.holder_collateral_tokens,
            fixture.holder_shard_tokens,
        ),
        &holder_keypair(),
    )
    .await;
    assert!(
        partial.accepted,
        "partial fractional redemption must land: {:?}",
        partial.result
    );
    assert_eq!(
        lamports_at(&mut context, fixture.holder).await,
        holder_lamports_before_partial,
        "a partial burn neither charges the holder nor closes record rent"
    );
    assert_eq!(
        token_amount(
            &account_at(&mut context, fixture.holder_shard_tokens)
                .await
                .expect("holder shards")
                .data
        ),
        OUTSTANDING_SHARDS - 2 * DENOMINATOR
    );
    assert_eq!(
        mint_supply(
            &account_at(&mut context, fixture.shard_mint)
                .await
                .expect("mint")
                .data
        ),
        OUTSTANDING_SHARDS - 2 * DENOMINATOR
    );
    assert_eq!(
        token_amount(
            &account_at(&mut context, fixture.holder_collateral_tokens)
                .await
                .expect("holder collateral")
                .data
        ),
        8
    );
    assert_eq!(
        token_amount(
            &account_at(&mut context, fixture.vault)
                .await
                .expect("vault")
                .data
        ),
        payout - 8
    );
    let partial_record = account_at(&mut context, record)
        .await
        .expect("partial record remains");
    assert_eq!(partial_record.lamports, record_rent);
    assert_eq!(
        FractionalClaimCheckV1::decode(&partial_record.data)
            .expect("partial record")
            .escrowed_atoms,
        payout - 8
    );
    assert_eq!(
        ClaimCheckEscrowV1::decode(
            &account_at(&mut context, fixture.escrow)
                .await
                .expect("escrow")
                .data
        )
        .expect("escrow")
        .outstanding_claim_checks,
        1
    );

    // Both halves are exactly twenty shards in the propagated 40-shard life.
    // They are distinct lifecycle acts, not a transaction replay: a real
    // validator naturally supplies a later blockhash, while ProgramTest keeps
    // the same bank alive until explicitly advanced. Move one real slot so the
    // identical economic request receives a fresh transaction identity.
    context
        .warp_to_slot(escrow.opened_slot + COMPACTION_DEADLINE_SLOTS_V1 + 1)
        .expect("advance one slot between equal-sized redemptions");
    let holder_lamports_before_settle = lamports_at(&mut context, fixture.holder).await;
    let settling = submit_holder_act(
        &mut context,
        "fractional claim-check: drive curve holder life [6]",
        fractional_redemption_instruction(
            &fixture,
            request(OUTSTANDING_SHARDS - 2 * DENOMINATOR),
            fixture.holder,
            fixture.holder_collateral_tokens,
            fixture.holder_shard_tokens,
        ),
        &holder_keypair(),
    )
    .await;
    assert!(
        settling.accepted,
        "settling fractional redemption must land: {:?}",
        settling.result
    );
    assert_eq!(lamports_at(&mut context, record).await, 0);
    assert_eq!(
        lamports_at(&mut context, fixture.holder).await,
        holder_lamports_before_settle + record_rent,
        "all live record lamports, including donations, return to the holder"
    );
    assert_eq!(
        token_amount(
            &account_at(&mut context, fixture.holder_collateral_tokens)
                .await
                .expect("holder collateral")
                .data
        ),
        payout
    );
    assert_eq!(
        token_amount(
            &account_at(&mut context, fixture.holder_shard_tokens)
                .await
                .expect("holder shards")
                .data
        ),
        0
    );
    assert_eq!(
        mint_supply(
            &account_at(&mut context, fixture.shard_mint)
                .await
                .expect("mint")
                .data
        ),
        0
    );
    assert_eq!(
        token_amount(
            &account_at(&mut context, fixture.vault)
                .await
                .expect("vault")
                .data
        ),
        0
    );
    assert_eq!(
        ClaimCheckEscrowV1::decode(
            &account_at(&mut context, fixture.escrow)
                .await
                .expect("escrow")
                .data
        )
        .expect("escrow")
        .outstanding_claim_checks,
        0
    );

    let vault_rent = lamports_at(&mut context, fixture.vault).await;
    let escrow_rent = lamports_at(&mut context, fixture.escrow).await;
    let cranker_before_close = lamports_at(&mut context, fixture.cranker).await;
    let close = submit_holder_act(
        &mut context,
        "fractional claim-check: drive curve holder life [7]",
        close_settled_escrow_instruction(&fixture),
        &cranker_keypair(),
    )
    .await;
    assert!(
        close.accepted,
        "settled escrow close must land: {:?}",
        close.result
    );
    assert_eq!(lamports_at(&mut context, fixture.vault).await, 0);
    assert_eq!(lamports_at(&mut context, fixture.escrow).await, 0);
    assert_eq!(
        lamports_at(&mut context, fixture.cranker).await,
        cranker_before_close + vault_rent + escrow_rent,
        "vault and escrow rent both reach the permissionless closer"
    );
    assert_eq!(
        token_amount(
            &account_at(&mut context, fixture.cranker_collateral_tokens)
                .await
                .expect("closer collateral account")
                .data
        ),
        0,
        "no collateral residue was stranded or swept"
    );
}

/// At `3/2`, the clamped quadratic has exact weights `[1/4,1/2,1/4]`.
/// Cumulative-floor at scale seven pays `[1,4,2]`; the full hostile lifecycle
/// proves the middle claim's four-atom rate physically.
#[tokio::test]
async fn a_degree_two_curve_compacts_and_redeems_without_stranding() {
    drive_curve_holder_life(CampaignBasisV1::CurvedDegreeTwo, &[1, 4, 2]).await;
}

/// At `3/2`, the clamped cubic has exact weights `[1/8,3/8,3/8,1/8]`.
/// Cumulative-floor at scale eleven pays `[1,4,4,2]`, while independent
/// per-term flooring would leave the final atom unpaid. The same real-ELF
/// hostile/partial/settling/close life proves that degree three reaches the
/// permissionless Fractional physical route without widening Structured's
/// authoritative K=3 child wall.
#[tokio::test]
async fn a_degree_three_curve_compacts_and_redeems_without_stranding() {
    drive_curve_holder_life(CampaignBasisV1::CurvedDegreeThree, &[1, 4, 4, 2]).await;
}

/// Create the Claims-role Custody replay cursor, by the real route.
///
/// NOT planted, and the reason is the one the sibling campaign states about the
/// same account: a Claims-role cursor is a prestate no route in this tree can
/// produce, so a fixture that wrote one would be asserting a shape nothing has
/// ever created. It is created here by executing
/// `custody_replay_v1`'s own route, which is also what makes the compaction's
/// Custody composition reachable at all -- `authenticate_custody_accounts`
/// refuses a cursor whose bytes are not exactly `CUSTODY_REPLAY_BYTES_V1`, and
/// an empty account is the shape a campaign gets for free and must not accept.
///
/// The payer is the CRANKER, not the holder. Somebody has to advance this rent
/// too, and the campaign's whole premise is that the party doing the work is a
/// stranger.
async fn create_custody_replay(context: &mut ProgramTestContext, fixture: &CampaignFixture) {
    use dclutch_claims_sbf::custody_replay_v1::expected_request_v1;
    use dclutch_claims_svm::custody_replay_v1::ClaimsCustodyReplayRequestV1;
    use dclutch_claims_svm::liability_basis_state_v2::LiabilityBasisMarketViewV2;
    use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};

    let aggregate = LiabilityBasisMarketViewV2::decode(&fixture.shared.claims_market_bytes)
        .expect("aggregate decode");
    let request = expected_request_v1(
        aggregate,
        CLAIMS_PROGRAM_ID.to_bytes(),
        fixture.cranker.to_bytes(),
        fixture.rent_credit.to_bytes(),
        Rent::default().minimum_balance(CUSTODY_REPLAY_BYTES_V1),
    )
    .expect("the sole Custody request this route sends");
    let request_bytes = request.to_bytes().expect("custody request bytes");
    let caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            fixture.release_set,
            fixture.shared.core_market.to_bytes(),
            ExecutionRoleV1::Claims,
            request.context,
            hash(&request_bytes).to_bytes(),
        )
        .expect("claims custody caller seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let instruction = Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(caller, false),
            AccountMeta::new_readonly(fixture.shared.core_market, false),
            AccountMeta::new_readonly(fixture.activation_cache, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
            AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
            AccountMeta::new_readonly(fixture.realm_record.raw, false),
            AccountMeta::new_readonly(fixture.realm_record.staging, false),
            AccountMeta::new(fixture.custody_replay, false),
            AccountMeta::new(fixture.cranker, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
            AccountMeta::new(fixture.rent_credit, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(fixture.shared.claims_market, false),
        ],
        data: ClaimsCustodyReplayRequestV1::new(fixture.shared.core_market.to_bytes())
            .expect("replay-creation request")
            .to_bytes()
            .to_vec(),
    };
    let outcome = submit_as(context,
        "fractional claim-check: create custody replay", instruction, &cranker_keypair()).await;
    assert!(
        outcome.accepted,
        "the Claims-role Custody replay must be creatable: {:?}",
        outcome.result
    );
}

/// Drive one campaign to the crank and return the outcome, whatever it is.
///
/// Shared by the admitted run and every hostile, so a hostile differs from the
/// admitted run in exactly the one fact its plant or its action names and in
/// nothing else. A hostile with its own setup path is a hostile that can pass
/// for the wrong reason.
async fn run_to_the_crank(
    plant: CampaignPlantV1,
    action: dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1,
) -> Outcome {
    run_to_the_crank_substituting(plant, action, CrankSubstitutionV1::None).await
}

/// What one hostile changes AFTER the escrow opened and the deadline elapsed.
///
/// Separate from [`CampaignPlantV1`] because a plant is a fact about how the
/// market was founded and this is a fact about the frame the crank is handed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CrankSubstitutionV1 {
    /// Nothing. The admitted run and every plant-driven hostile.
    None,
    /// The Core Market has not reached a terminal phase.
    ///
    /// Written at the Core state's own derived address, decoded and re-encoded
    /// from the fixture's own bytes, so its identity, its owner, its length and
    /// every seed it derives under are byte-identical to the admitted run and
    /// the phase is the sole discriminator. `CoreState::encode` refuses a
    /// non-terminal phase carrying a terminal receipt, so dropping the receipt
    /// is not a second substitution -- it is what an Open Market IS.
    CoreMarketBeforeTerminal,
}

async fn run_to_the_crank_substituting(
    plant: CampaignPlantV1,
    action: dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1,
    substitution: CrankSubstitutionV1,
) -> Outcome {
    use dclutch_claims_svm::{
        fractional_claim_check_compaction_request_v1::{
            FractionalCompactToClaimCheckRequestV1, FractionalCompactionCoordinatesV1,
        },
        liability_basis_state_v2::LiabilityBasisMarketViewV2,
    };
    let (test, fixture) = campaign_fixture_planting(plant);
    let mut context = test.start_with_context().await;
    let opened = submit_as(&mut context,
        "fractional claim-check: run to the crank substituting", open_instruction(&fixture), &opener_keypair()).await;
    assert!(opened.accepted, "the open must land: {:?}", opened.result);
    create_custody_replay(&mut context, &fixture).await;
    let escrow = ClaimCheckEscrowV1::decode(
        &account_at(&mut context, fixture.escrow)
            .await
            .expect("escrow")
            .data,
    )
    .expect("escrow decodes");
    let market_revision = LiabilityBasisMarketViewV2::decode(&fixture.shared.claims_market_bytes)
        .expect("aggregate decode")
        .revision;
    let terminal = crank_terminal_request(&fixture, market_revision);
    let (payout, custody_caller) = crank_payout_and_caller(&fixture, terminal, market_revision);
    let request = FractionalCompactToClaimCheckRequestV1::new(
        FractionalCompactionCoordinatesV1 {
            terms: fixture.terms_record.digest,
            token_behavior: fixture.behavior_record.digest,
            expected_root_revision: ROOT_REVISION,
            representation_coordinate: fixture.outcome,
            payout_per_claim: payout / (OUTSTANDING_SHARDS / DENOMINATOR),
        },
        crank_terminal_request(&fixture, market_revision),
    )
    .expect("canonical fractional compaction request");
    context
        .warp_to_slot(escrow.opened_slot + COMPACTION_DEADLINE_SLOTS_V1)
        .expect("warp past the compaction deadline");
    if substitution == CrankSubstitutionV1::CoreMarketBeforeTerminal {
        use dclutch_market_core_codec::{CoreState, Phase as CorePhase};
        let mut core = CoreState::decode(&fixture.shared.core_state).expect("Core state decodes");
        core.phase = CorePhase::Open;
        core.terminal_winner = 0;
        core.terminal_receipt = None;
        let mut account = account_at(&mut context, fixture.shared.core_market)
            .await
            .expect("Core Market");
        account.data = core.encode().expect("open Core state").to_vec();
        context.set_account(
            &fixture.shared.core_market,
            &solana_account::AccountSharedData::from(account),
        );
    }
    submit_as(
        &mut context,
        "fractional claim-check: run to the crank substituting [2]",
        compaction_instruction(&fixture, &request, custody_caller, action),
        &cranker_keypair(),
    )
    .await
}

/// **The ruled fiftieth account, red-proved: a RentCredit that does not derive
/// is refused by name, and nothing else about the market changed.**
///
/// WAVE `b4546291` added the Rent program so `authenticate_rent_credit` could
/// run. This is the witness that it runs and that it bites. The planted credit
/// decodes, carries this market's own market, release set and generation, and
/// sits at exactly the address the reserve's admission pins -- so every conjunct
/// the route made BEFORE the ruling still passes on it. The single thing wrong
/// is that `create_program_address` over the record's own persisted seeds no
/// longer reproduces its address, which is precisely and only what the fiftieth
/// account made checkable.
///
/// Non-vacuous by construction: the control is the admitted campaign above,
/// which is the same fixture with the same everything and a correct bump, and
/// it lands.
#[tokio::test]
async fn a_rent_credit_that_does_not_derive_is_refused_by_name() {
    let outcome = run_to_the_crank(
        CampaignPlantV1::RentCreditAtANonDerivedAddress,
        dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1::Signed,
    )
    .await;
    assert!(
        !outcome.accepted,
        "a non-derived RentCredit must be refused"
    );
    assert_eq!(
        custom_refusal(&outcome.result),
        Some(
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckCompactionSbfErrorV1::Rent
                as u32,
        ),
        "and it must be refused as Rent, by name -- not folded into Identity, so a \
         validator log can tell 'your rent is going somewhere else' from a mistyped escrow"
    );
}

/// The terminal certificate hostile reaches the inner settlement and returns
/// the fractional route's exact `TerminalIdentity` code.
///
/// This is one of the two failures FRACCHECK-7 could diagnose only by
/// temporarily deleting the old `map_err(|_| Economic)`: the certificate is a
/// valid record at the Core-pinned address, but its decoded Market identity
/// disagrees. The faithful campaign proves the same route with the correct
/// field commits.
#[tokio::test]
async fn a_terminal_identity_refusal_is_not_flattened_to_economic() {
    let outcome = run_to_the_crank(
        CampaignPlantV1::TerminalCertificateIdentityMismatch,
        dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1::Signed,
    )
    .await;
    assert!(!outcome.accepted, "the foreign certificate must refuse");
    assert_eq!(
        custom_refusal(&outcome.result),
        Some(
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckCompactionSbfErrorV1::TerminalIdentity
                as u32,
        ),
        "the validator-visible code must name the inner terminal identity failure"
    );
}

/// The SignedDelta hostile reaches the generated packet and returns the
/// fractional route's exact `SignedDeltaRelease` code.
///
/// The plan says `CallerRole::Claims`, whose caller coordinates must equal the
/// Claims deployment. Substituting the activated Trading test caller changes
/// only that joined role and reproduces the other inner failure FRACCHECK-7 had
/// to expose by hand.
#[tokio::test]
async fn a_signed_delta_release_refusal_is_not_flattened_to_economic() {
    let outcome = run_to_the_crank(
        CampaignPlantV1::SignedDeltaCallerReleaseMismatch,
        dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1::Signed,
    )
    .await;
    assert!(!outcome.accepted, "the wrong caller role must refuse");
    assert_eq!(
        custom_refusal(&outcome.result),
        Some(
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckCompactionSbfErrorV1::SignedDeltaRelease
                as u32,
        ),
        "the validator-visible code must name the inner SignedDelta release failure"
    );
}

/// **Witness w8: a compaction without Trading dies, and it dies EARLIER than
/// §17.8 predicted.**
///
/// The ruling reasoned that a stranger arriving without Trading would reach the
/// `SetAuthority` hand-off and be refused there by Token-2022 for want of the
/// current authority's signature. It does not get that far: the frame guard
/// refuses an unsigned root at `role_account`, before any derivation, any
/// decode and any CPI. Refusing earlier is strictly better -- the transaction
/// costs the runtime nothing and the refusal names the frame rather than a
/// Token-2022 error a reader has to work backwards from -- and the ruling's
/// conclusion is unchanged: Trading-composition is enforced by the root signer
/// alone. Recorded as observed rather than as predicted.
#[tokio::test]
async fn a_compaction_without_trading_is_refused_at_the_frame() {
    let outcome = run_to_the_crank(
        CampaignPlantV1::Faithful,
        dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1::UnsignedRoot,
    )
    .await;
    assert!(
        !outcome.accepted,
        "a compaction whose root never signed must be refused"
    );
    assert_eq!(
        custom_refusal(&outcome.result),
        Some(
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckCompactionSbfErrorV1::Authority
                as u32,
        ),
        "refused as Authority, at the frame"
    );
}

/// **A stranger cannot choose the denominator every returning holder is paid
/// by: exposure terms this Market never selected are refused BY NAME.**
///
/// The hole, measured rather than reasoned about. Before
/// `authenticate_selected_config`, this route's only joins on the finalized
/// terms were `terms.market()` and `terms.release_set()` -- two fields the
/// record's own author writes -- plus a digest join against a value the caller
/// supplies. Registry publication is permissionless. The terms record is the
/// SOLE author of the `denominator` the route persists into a fractional
/// claim-check, which is the divisor every returning holder's payout is
/// computed with for the life of the record. The conservation plan does not
/// bound it: it requires `entitlement == (supply / D) * rate`, and `D = supply`
/// with `rate = entitlement` satisfies that exactly for any supply.
///
/// The three sibling Fractional routes -- `fractional_atomic_v3.rs:253` and
/// `:738`, `fractional_retirement_v3.rs:607` -- each derive the selection-config
/// identity from the authenticated terms and require it to equal the config the
/// founded Market's capability manifest chose. The permissionless one did not.
///
/// **Proven red first.** Against the ELF before the guard this campaign is
/// ACCEPTED: a Market whose manifest selected some other config still compacts
/// against whatever terms the cranker presents. That is the whole finding, and
/// it is why the assertion here is on the exact discriminant and not on
/// `is_err()` -- a compaction can refuse for a dozen reasons and only one of
/// them is this one.
#[tokio::test]
async fn exposure_terms_the_market_never_selected_are_refused_by_name() {
    let outcome = run_to_the_crank(
        CampaignPlantV1::UnselectedExposureTerms,
        dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1::Signed,
    )
    .await;
    assert!(
        !outcome.accepted,
        "terms the Market's manifest never selected must be refused"
    );
    assert_eq!(
        custom_refusal(&outcome.result),
        Some(
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckCompactionSbfErrorV1::SelectionConfig
                as u32,
        ),
        "refused as SelectionConfig, by name -- not folded into Terms, because \
         'these are not the terms this Market selected' is a different accusation \
         from 'this record does not decode or names another Market'"
    );
}

/// **`0x5644` stops being a refusal this family publishes and nothing can
/// raise: a compaction before Terminal is refused as `Phase`.**
///
/// The asymmetry this closes. The native twin reads Core state itself and
/// refuses `Phase` on both conjuncts (`claim_check_compaction_v1.rs:437-448`),
/// saying why: *a checked invariant is one an implementer cannot silently
/// delete*. The fractional twin declared the identical code and read no Core
/// state at all, so `Phase` had ZERO raise sites in the whole program.
///
/// It was not an unguarded route, and that distinction is worth keeping exact:
/// the wrapped `terminal_settlement_v3::authenticate_core` admits only
/// `TerminalOrRetiring` and its caller requires `core.terminal_receipt`. What
/// it does not do is say so in this family's vocabulary -- both refuse as
/// `ClaimsSbfError::Identity`, which `fractional_terminal_refusal` folds to
/// `TerminalIdentity`. So a crank turned on a Market that had not resolved was
/// told its identities disagreed, and the code that names the real cause could
/// never appear in a validator log.
///
/// **Proven red first**: before the guard this returns `TerminalIdentity`
/// (`0x564F`), not `Phase` (`0x5644`).
///
/// The 180-day deadline is untouched: the campaign still warps past the real
/// `COMPACTION_DEADLINE_SLOTS_V1` before it cranks, exactly as the admitted run
/// does. The only thing this hostile changes is the Market's phase.
#[tokio::test]
async fn a_compaction_before_terminal_is_refused_as_phase() {
    let outcome = run_to_the_crank_substituting(
        CampaignPlantV1::Faithful,
        dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1::Signed,
        CrankSubstitutionV1::CoreMarketBeforeTerminal,
    )
    .await;
    assert!(
        !outcome.accepted,
        "a compaction on a Market before Terminal must be refused"
    );
    assert_eq!(
        custom_refusal(&outcome.result),
        Some(
            dclutch_claims_sbf::fractional_claim_check_v1::FractionalClaimCheckCompactionSbfErrorV1::Phase
                as u32,
        ),
        "refused as Phase, by name -- the code this family already published"
    );
}

/// **A worthless coordinate is compacted, mints NO record, and the burn
/// authority never moves.**
///
/// The other side of a weld the campaign above only exercises in one direction.
/// `FractionalClaimCheckCompactionPlanV1` requires
/// `mints_claim_check() == (escrowed_atoms != 0)` in BOTH directions, and the
/// route welds the `SetAuthority` hand-off to the same bit. So a market that
/// resolved somewhere else leaves this coordinate's shard holders with nothing
/// to claim, and the correct behaviour is: sweep the Position, write no record,
/// and leave `PermissionedBurn` exactly where it was.
///
/// **Why the hand-off half is the one worth having.** A record that is minted
/// for a zero claim is a wasted account. An authority that is handed off for a
/// zero claim is worse and is not recoverable: the root can never sign again
/// after retirement, so a burn authority moved to an escrow that will never
/// hold a claim is a Mint whose shards nobody -- holder, escrow or root -- can
/// ever burn. The route's own comment says moving a live authority to serve a
/// claim that does not exist would be "an authority moved for nothing"; this is
/// that sentence, executed.
///
/// The zero side also takes a different Custody path (no CPI, no revision
/// bump, and coordinate 23 is literally the Claims program), so this is not
/// merely the paying campaign with a smaller number -- it is the branch the
/// paying campaign never enters.
#[tokio::test]
async fn a_worthless_coordinate_mints_no_record_and_never_moves_the_burn_authority() {
    use dclutch_claims_svm::{
        fractional_claim_check_compaction_request_v1::{
            FractionalCompactToClaimCheckRequestV1, FractionalCompactionCoordinatesV1,
        },
        liability_basis_state_v2::LiabilityBasisMarketViewV2,
    };
    use dclutch_fractional_compaction_test_caller_sbf::FractionalCompactionCallerActionV1;

    // The market resolved to a coordinate that is NOT the reserve's.
    const ELSEWHERE: u32 = 1;
    assert_ne!(ELSEWHERE, OUTCOME);
    let (test, fixture) = campaign_fixture_full(CampaignPlantV1::Faithful, ELSEWHERE);
    let mut context = test.start_with_context().await;

    let opened = submit_as(&mut context,
        "fractional claim-check: a worthless coordinate mints no record and never moves the burn authority", open_instruction(&fixture), &opener_keypair()).await;
    assert!(opened.accepted, "the open must land: {:?}", opened.result);
    create_custody_replay(&mut context, &fixture).await;
    let escrow = ClaimCheckEscrowV1::decode(
        &account_at(&mut context, fixture.escrow)
            .await
            .expect("escrow")
            .data,
    )
    .expect("escrow decodes");

    let market_revision = LiabilityBasisMarketViewV2::decode(&fixture.shared.claims_market_bytes)
        .expect("aggregate decode")
        .revision;
    let terminal = crank_terminal_request(&fixture, market_revision);
    let (payout, custody_caller) = crank_payout_and_caller(&fixture, terminal, market_revision);
    assert_eq!(
        payout, 0,
        "a coordinate the market resolved away from pays nothing"
    );

    let request = FractionalCompactToClaimCheckRequestV1::new(
        FractionalCompactionCoordinatesV1 {
            terms: fixture.terms_record.digest,
            token_behavior: fixture.behavior_record.digest,
            expected_root_revision: ROOT_REVISION,
            representation_coordinate: OUTCOME,
            // NONZERO, and the wire insists on it: a zero rate is refused as
            // `InvalidEntitlement` because it "promises a record nobody would
            // ever redeem". What makes the escrow empty here is not the rate
            // but the SUPPLY -- zero outstanding shards form zero whole claims,
            // so `whole_claims * rate` is zero whatever the rate is. Reaching
            // the plan's no-claim branch through supply rather than through
            // rate is the only route the protocol actually permits.
            payout_per_claim: 1,
        },
        crank_terminal_request(&fixture, market_revision),
    )
    .expect("canonical fractional compaction request");

    context
        .warp_to_slot(escrow.opened_slot + COMPACTION_DEADLINE_SLOTS_V1)
        .expect("warp past the compaction deadline");

    let record = record_address(&fixture);
    let position = fixture.shared.reserve_position.account;
    let before_hoard = token_amount(
        &account_at(&mut context, fixture.hoard)
            .await
            .expect("hoard")
            .data,
    );
    let before_mint = account_at(&mut context, fixture.shard_mint)
        .await
        .expect("shard mint")
        .data;
    let before_holder = lamports_at(&mut context, fixture.holder).await;

    let outcome = submit_as(
        &mut context,
        "fractional claim-check: a worthless coordinate mints no record and never moves the burn authority [2]",
        compaction_instruction(
            &fixture,
            &request,
            custody_caller,
            FractionalCompactionCallerActionV1::Signed,
        ),
        &cranker_keypair(),
    )
    .await;
    assert!(
        outcome.accepted,
        "compacting a worthless coordinate must still land -- the market has to be \
         able to finish retiring: {:?} (code {:?})",
        outcome.result,
        custom_refusal(&outcome.result)
    );

    // NO COLLATERAL MOVED.
    let after_hoard = token_amount(
        &account_at(&mut context, fixture.hoard)
            .await
            .expect("hoard")
            .data,
    );
    let after_vault = token_amount(
        &account_at(&mut context, fixture.vault)
            .await
            .expect("vault")
            .data,
    );
    assert_eq!(
        after_hoard, before_hoard,
        "a zero payout must move no collateral"
    );
    assert_eq!(after_vault, 0, "and the vault must still be empty");

    // NO RECORD.
    assert_eq!(
        lamports_at(&mut context, record).await,
        0,
        "a coordinate with nothing to claim must mint no claim-check"
    );

    // AND THE BURN AUTHORITY NEVER MOVED -- asserted over the Mint's WHOLE
    // bytes rather than over the extension a reader would think to check, so a
    // hand-off that touched any part of the Mint reds this.
    let after_mint = account_at(&mut context, fixture.shard_mint)
        .await
        .expect("shard mint")
        .data;
    assert_eq!(
        after_mint, before_mint,
        "the PermissionedBurn authority must still be the root: an authority handed \
         to an escrow that will never hold a claim is a Mint nobody can ever burn, \
         and after retirement the root cannot hand it back"
    );

    // The Position is still swept -- which is the whole point of compacting a
    // worthless coordinate at all.
    assert_eq!(lamports_at(&mut context, position).await, 0);
    assert_eq!(
        lamports_at(&mut context, fixture.reserve_admission).await,
        0
    );
    assert_eq!(
        lamports_at(&mut context, fixture.holder).await,
        before_holder
    );

    println!(
        "ZERO-PAYOUT COMPACTION: units={} hoard unmoved={} record minted=no mint bytes changed=no",
        outcome.units, after_hoard
    );
}
