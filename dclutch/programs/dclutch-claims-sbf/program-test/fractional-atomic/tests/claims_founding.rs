//! The first Claims founding executed against a real ELF.
//!
//! Nothing in this tree had ever run one. `found_program_test.rs` drives Core's
//! Found stage to `Founding + Prepaid` and stops there; every Claims
//! program-test genesis-plants the aggregate with
//! `encode_liability_basis_market_v2` instead of founding it; and the only
//! executor of `DCLFDR05` is the local-validator bootstrap, against a live
//! cluster. So the route that CREATES every Claims aggregate — and, since
//! CLAIMS-17, seats the failure escrow — had no fixture-level evidence at all.
//!
//! # What had to exist first
//!
//! One thing, and it is why this was never written: `authenticate_authority`
//! requires the frame's account 0 to be the PDA
//! `CallerAuthoritySeedsV1(release_set, market, Trading,
//! founding_intent_digest, request_digest)` addresses **under the request's own
//! trading program**, and `invoke_signed` signs only for the calling program's
//! own addresses. `programs/dclutch-claims-sbf/test-programs/founding-caller`
//! is that caller. It declares no refusal code of its own on purpose, so a
//! founding's refusal reaches this file as the founding's own discriminant
//! rather than as one wrapper code covering thirty-three account conjuncts.
//!
//! # The prestate is forged, and every digest that binds it is stated here
//!
//! The permit is PLANTED as canonical Core-owned bytes rather than issued by
//! Core's Found stage. That is the fixture line this campaign draws, and it is
//! drawn where the tree already draws it — `narrow_fixture` plants the Core
//! state and the Product graph for the same reason. What is NOT weakened is the
//! join: the permit's `FoundingIntentV5` still has to satisfy every one of
//! `authenticate_permit_body`'s named conjuncts, both projected-custody
//! receipts still have to hash to what the request commits to, and the Custody
//! replay cursor still has to name the realization receipt's own digest. The
//! chain this file builds, in dependency order, is:
//!
//! ```text
//!   ticket_context ─hashv(PROJECTED_HOARD_CONTEXT_DOMAIN_V1)→ projected_context
//!   lock receipt ─hash→ request.custody_receipt_digest
//!   core state   ─hash→ projected_receipt.market_state_digest
//!   projected receipt ─hash→ replay.last_poststate_commitment
//!   intent ─hash→ request.founding_intent_digest = permit.claims_intent_digest
//!   request ─hash→ permit.claims_request_digest, and the authority PDA's last seed
//! ```
//!
//! Nothing in that chain is asserted; each link is computed from the previous
//! one, so a fixture that drifts refuses rather than passing.
//!
//! # CU per stage, and a correction to the headline number
//!
//! Measured at `a514cace` on an ELF built `--features claims-cu-profile`, which
//! is a DIFFERENT ARTIFACT from the one the campaigns above run: its totals are
//! 234,825 and 200,941 against the shipped 240,040 and 209,160. Only the shape
//! below is evidence; the two totals to quote are the shipped ones.
//!
//! | stage | refunding | categorical | difference |
//! |---|---|---|---|
//! | `found-frame` (decode, parse, privileges) | 7,388 | 7,388 | 0 |
//! | `found-authority` | 3,563 | 2,063 | +1,500 |
//! | `found-releases` (the four-role loop) | 47,165 | 47,165 | 0 |
//! | `found-permit` (permit, both receipts) | 5,350 | 5,350 | 0 |
//! | `found-custody` (poststate, replay) | 2,044 | 2,044 | 0 |
//! | `found-product-core` (record walk) | 47,159 | 35,154 | +12,005 |
//! | `found-rent-vacancy` (+ escrow seating) | 26,073 | 26,070 | +3 |
//! | `found-candidates` | 17,101 | 13,850 | +3,251 |
//! | `found-allocate` (System CPIs) | 25,850 | 14,328 | +11,522 |
//! | `found-commit` | 10,469 | 6,366 | +4,103 |
//! | enter to commit | 192,162 | 159,778 | +32,384 |
//!
//! **THE 30,880 IS NOT ALL ESCROW, and the table is what says so.** The escrow's
//! own work is `found-candidates` + `found-allocate` + `found-commit` — the
//! second Position and admission built, allocated over four more System CPIs,
//! and copied — and that is about **18,900**. `found-authority`'s +1,500 and
//! `found-product-core`'s +12,005 are `find_program_address` iteration variance:
//! the two campaigns carry different request and record digests, so their bump
//! searches take different numbers of turns, and a turn costs about 1,500. A
//! lane that quoted the whole difference as the escrow's price would be pricing
//! the fixture's digests.
//!
//! `found-releases` is the other number worth having: **47,165 CU, a quarter of
//! this route's own consumption, and identical under both shapes.** The route's
//! own comment asks for exactly this quantity — "`found-releases` minus
//! `found-authority` is that question's answer for this route" — because a
//! Claims child is one `consumed` line in its caller's log, and one number
//! cannot say whether it was spent on work only Claims can do or on
//! re-establishing a release set its caller had already established. It is the
//! latter, and it is the single largest stage.
//!
//! # Both shapes, over one fixture
//!
//! The categorical founding and the refunding one differ in exactly one input —
//! the basis record's payout scale, `1` against `basis_width - 1` — because
//! `categorical_refunds_on_failure_v3` reads the shape off the RECORD and no
//! caller states it. Everything else, including all thirty-three accounts, is
//! identical. That is decision 0025 item 2's claim made checkable: the escrow
//! accounts ride on both frames, and only the refunding founding allocates them.

use std::{env, fs, path::PathBuf};

use dclutch_claims::{
    founding_v5::{
        CLAIMS_FOUNDING_ACCOUNT_COUNT_V6, CLAIMS_FOUNDING_RECEIPT_BYTES_V5,
        CLAIMS_FOUNDING_REQUEST_BYTES_V5, ClaimsFoundingAggregateSeedsV5, ClaimsFoundingReceiptV5,
        ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5,
    },
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2, liability_basis_vector_width_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionSeedsV2,
    },
};
use dclutch_custody::{
    CallerRoleV1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyVaultSeedsV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCustodyLockReceiptV1,
    ProjectedCustodyReceiptV1,
};
use dclutch_fractional_atomic_program_test::{
    campaign_support::{
        ReleaseSetInputV1, activation_cache, add_account, add_account_with_lamports,
        add_upgradeable_program, collateral_mint_bytes, finalized, programdata_address,
        token_account_bytes_for, token_program_id,
    },
    narrow_fixture::{
        NarrowBasisInputV3, NarrowFixtureInputV2, NarrowFixtureV2, compile_narrow_fixture_v3,
    },
};
use dclutch_market::{
    CoreState, FoundingIntentV5, Identity, Phase, Readiness, SeriesFoundingPermitV1,
};
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_market::rent::RefundAuthority;
use dclutch_market::rent::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
};
use dclutch_custody::token_svm::{PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID};
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
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
/// The Trading role, which for this campaign is the founding caller.
const CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa9; 32]);
const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xb6; 32]);
const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0x74; 32]);
const FUNDING_SOURCE: Pubkey = Pubkey::new_from_array([0x66; 32]);
const REFUND_WALLET: [u8; 32] = [0x5f; 32];
const RENT_BENEFICIARY: Pubkey = Pubkey::new_from_array([0x43; 32]);

const CLAIM_COUNT: u32 = 4;
const GENERATION: u64 = 41;
const CUSTODY_CONTEXT: [u8; 32] = [0x62; 32];
const TICKET_CONTEXT: [u8; 32] = [0x71; 32];
const PARENT_ROOT: [u8; 32] = [0x72; 32];
const SERIES_SOURCE: [u8; 32] = [0x73; 32];
/// The digest of the Realize request Custody consumed. It only has to be
/// nonzero and DIFFERENT from the Lock request's, because the intent carries
/// the projected pair and the request the realized one.
const REALIZE_REQUEST_DIGEST: [u8; 32] = [0x81; 32];
const LOCK_REQUEST_DIGEST: [u8; 32] = [0x83; 32];
const EXPIRY_SLOT: u64 = 10_000;
/// Complete sets the founding issues.
const QUANTITY: u64 = 7;
/// The projected replay revision the Lock stepped to.
const PROJECTED_RESULTING_REVISION: u64 = 3;
/// PINNED TO ONE BY THE INTENT ITSELF. `validate_coordinates` refuses any other
/// value, which in turn pins the request's post-custody revision to one and its
/// pre-custody revision to zero -- a founding's Custody cursor has taken exactly
/// one step, the Realize, and there is no earlier normal act to have taken
/// another.
const NORMAL_REPLAY_REVISION: u64 = 1;

fn founder_keypair() -> Keypair {
    Keypair::new_from_array([0x31; 32])
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
        caller: read("dclutch_claims_founding_test_caller_sbf.so"),
    }
}

// ---------------------------------------------------------------------------
// The founding world
// ---------------------------------------------------------------------------

/// Which shape the RECORD says this Market is. Nothing else differs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FoundingShapeV1 {
    /// Payout scale 1: a categorical Market, which seats no escrow.
    Categorical,
    /// Payout scale `basis_width - 1`: a refunding Market, which seats one.
    Refunding,
}

impl FoundingShapeV1 {
    fn basis(self) -> NarrowBasisInputV3<'static> {
        match self {
            Self::Categorical => NarrowBasisInputV3::Categorical,
            Self::Refunding => NarrowBasisInputV3::CategoricalRefunding,
        }
    }

    fn seats_escrow(self) -> bool {
        matches!(self, Self::Refunding)
    }
}

/// What this campaign deliberately gets wrong, if anything.
///
/// Both arms perturb ONE thing about the refunding world and leave the other
/// thirty-two accounts, the whole digest chain and the request untouched, so
/// the refusal each names is the conjunct under test and not a length check
/// three stages earlier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostileV1 {
    /// Nothing: the founding this campaign expects to be accepted.
    None,
    /// The escrow Position is a Position of the same aggregate, owned by
    /// somebody else. Every seed helper still succeeds; only the derivation
    /// disagrees.
    EscrowIsNotTheMarketsOwn,
    /// The escrow's accounts are the Market's own and are left UNFUNDED, as a
    /// categorical founding may leave them.
    EscrowRentNotPrepaid,
}

struct FoundingWorld {
    shared: NarrowFixtureV2,
    core_state: Vec<u8>,
    activation_cache: Pubkey,
    release_set: [u8; 32],
    realm_raw: Pubkey,
    realm_staging: Pubkey,
    permit: Pubkey,
    aggregate: Pubkey,
    position: Pubkey,
    admission: Pubkey,
    escrow_position: Pubkey,
    escrow_admission: Pubkey,
    hoard: Pubkey,
    custody_replay: Pubkey,
    rent_credit: Pubkey,
    request: ClaimsFoundingRequestV5,
    instruction_data: Vec<u8>,
    caller_authority: Pubkey,
    aggregate_rent: u64,
    position_rent: u64,
    admission_rent: u64,
}

fn world(shape: FoundingShapeV1, hostile: HostileV1) -> (ProgramTest, FoundingWorld) {
    let artifacts = artifacts();
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
            "dclutch_claims_founding_test_caller_sbf",
            CALLER_PROGRAM_ID,
            artifacts.caller.as_slice(),
        ),
    ] {
        add_upgradeable_program(&mut test, name, program, elf);
    }
    test.add_program("spl_token_2022", token_program_id(), None);

    let (release_set, cache_bytes) = activation_cache(&ReleaseSetInputV1 {
        core: (CORE_PROGRAM_ID, artifacts.core.as_slice()),
        claims: (CLAIMS_PROGRAM_ID, artifacts.claims.as_slice()),
        trading: (CALLER_PROGRAM_ID, artifacts.caller.as_slice()),
        custody: Some((CUSTODY_PROGRAM_ID, artifacts.custody.as_slice())),
    });
    let activation_cache_key = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
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
    let realm_record = finalized(
        REGISTRY_PROGRAM_ID,
        REALM_SCHEMA_RELEASE_ID_V1,
        RealmV1::new(RealmV1Input {
            token_program: TOKEN_2022_PROGRAM_ID,
            collateral_mint: COLLATERAL_MINT.to_bytes(),
            collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("canonical Realm")
        .to_bytes()
        .to_vec(),
    );

    let founder = founder_keypair().pubkey();
    let shared = compile_narrow_fixture_v3(
        NarrowFixtureInputV2 {
            outcome_count: usize::try_from(CLAIM_COUNT).expect("width"),
            registry_program: REGISTRY_PROGRAM_ID,
            core_program: CORE_PROGRAM_ID,
            claims_program: CLAIMS_PROGRAM_ID,
            release_set,
            realm_id: realm_record.digest,
            custody_context: CUSTODY_CONTEXT,
            generation: GENERATION,
            actor_owner: founder,
            reserve_owner: Pubkey::new_from_array([0x32; 32]),
            funded_coordinate: 0,
            funded_balance: 0,
            position_revision: 0,
            reserve_balance: 0,
            terminal: None,
            rent_beneficiary: RENT_BENEFICIARY,
            graph_id: [0x34; 32],
            exposure_id: [0x35; 32],
        },
        shape.basis(),
    )
    .expect("narrow founding fixture");

    // The Market's permanent rent beneficiary IS its RentCredit, which
    // `authenticate_rent_credit` compares for exact equality. The credit's
    // address derives from the Market and the generation, and the Market's own
    // address derives from `MarketIdentity`, which carries neither the
    // beneficiary nor the phase -- so this is a state edit, not a re-address.
    let (rent_credit, rent_bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            shared.core_market.as_ref(),
            &GENERATION.to_le_bytes(),
        ],
        &RENT_PROGRAM_ID,
    );

    // A FOUNDING CONSUMES A MARKET IN `Phase::Founding`, which is what Core's
    // own Found stage leaves behind and what `narrow_fixture` -- built for
    // campaigns that open AFTER founding -- does not. The phase is re-stated
    // through the codec that owns `CoreState` rather than by poking byte ten.
    let mut core = CoreState::decode(&shared.core_state).expect("fixture Core state");
    core.phase = Phase::Founding;
    core.readiness = Readiness::Prepaid;
    core.rent_beneficiary = Identity::new(rent_credit.to_bytes()).expect("rent beneficiary");
    let core_state = core.encode().expect("Founding-phase Core state").to_vec();
    let core_digest = hash(&core_state).to_bytes();

    let collateral_atoms = QUANTITY
        .checked_mul(shared.payout_scale)
        .expect("collateral atoms");

    // ---- addresses -------------------------------------------------------
    let aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(shared.core_market.to_bytes())
            .expect("aggregate seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), founder.to_bytes())
            .expect("position seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), founder.to_bytes())
            .expect("admission seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let escrow_owner = Pubkey::find_program_address(
        &ProtocolPositionClaimsCapabilitySeedsV2::new(
            shared.core_market.to_bytes(),
            CLAIM_COUNT - 1,
        )
        .expect("escrow owner seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    // A Position of the SAME aggregate under a different owner: the seeds are
    // well-formed and the account is vacant, so nothing before the escrow
    // derivation can notice.
    let escrow_position_owner = match hostile {
        HostileV1::EscrowIsNotTheMarketsOwn => Pubkey::new_from_array([0x39; 32]),
        HostileV1::None | HostileV1::EscrowRentNotPrepaid => escrow_owner,
    };
    let escrow_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), escrow_position_owner.to_bytes())
            .expect("escrow position seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let escrow_admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), escrow_owner.to_bytes())
            .expect("escrow admission seeds")
            .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(shared.core_market.to_bytes(), release_set).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let projected_context =
        hashv(&[&PROJECTED_HOARD_CONTEXT_DOMAIN_V1[..], &TICKET_CONTEXT[..]]).to_bytes();
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            shared.core_market.to_bytes(),
            release_set,
            projected_context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let custody_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            shared.core_market.to_bytes(),
            release_set,
            CallerRoleV1::Trading,
            projected_context,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let (permit, permit_bump) = Pubkey::find_program_address(
        &[
            &dclutch_market::SERIES_FOUNDING_PERMIT_PDA_DOMAIN_V1[..],
            &release_set,
            &shared.core_market.to_bytes(),
            &TICKET_CONTEXT,
        ],
        &CORE_PROGRAM_ID,
    );

    // ---- the digest chain ------------------------------------------------
    let lock_receipt = ProjectedCustodyLockReceiptV1 {
        market: shared.core_market.to_bytes(),
        release_set,
        context_digest: projected_context,
        source_vault: FUNDING_SOURCE.to_bytes(),
        source_replay: [0x84; 32],
        hoard_vault: hoard.to_bytes(),
        rent_credit: rent_credit.to_bytes(),
        request_digest: LOCK_REQUEST_DIGEST,
        amount: collateral_atoms,
        source_vault_rent_lamports: 2_039_280,
        source_replay_rent_lamports: 2_039_280,
        resulting_revision: PROJECTED_RESULTING_REVISION - 1,
    };
    let lock_bytes = lock_receipt.encode().expect("lock receipt bytes");
    let lock_digest = hash(&lock_bytes).to_bytes();

    let projected_receipt = ProjectedCustodyReceiptV1 {
        realized: true,
        aborted_open: false,
        market: shared.core_market.to_bytes(),
        release_set,
        parent_capability_root: PARENT_ROOT,
        context_digest: projected_context,
        hoard_vault: hoard.to_bytes(),
        amount: collateral_atoms,
        request_digest: REALIZE_REQUEST_DIGEST,
        market_state_digest: core_digest,
        rent_credit: rent_credit.to_bytes(),
        resulting_revision: PROJECTED_RESULTING_REVISION,
    };
    let projected_bytes = projected_receipt
        .encode()
        .expect("realization receipt bytes");
    let projected_digest = hash(&projected_bytes).to_bytes();

    let identity = |bytes: [u8; 32]| Identity::new(bytes).expect("nonzero identity");
    let intent = FoundingIntentV5::new(
        permit_bump,
        identity(release_set),
        identity(shared.core_market.to_bytes()),
        identity(shared.product.digest),
        identity(SERIES_SOURCE),
        identity(founder.to_bytes()),
        identity(TICKET_CONTEXT),
        identity(PARENT_ROOT),
        identity(custody_replay.to_bytes()),
        identity(FUNDING_SOURCE.to_bytes()),
        identity(hoard.to_bytes()),
        identity(REALIZE_REQUEST_DIGEST),
        identity(projected_digest),
        identity(CALLER_PROGRAM_ID.to_bytes()),
        identity(CLAIMS_PROGRAM_ID.to_bytes()),
        identity(rent_credit.to_bytes()),
        GENERATION,
        QUANTITY,
        shared.payout_scale,
        EXPIRY_SLOT,
        PROJECTED_RESULTING_REVISION,
        NORMAL_REPLAY_REVISION,
    )
    .expect("founding intent");
    let intent_digest = hash(&intent.encode().expect("intent bytes")).to_bytes();

    let rent = Rent::default();
    let aggregate_rent = rent.minimum_balance(
        liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, CLAIM_COUNT)
            .expect("aggregate width"),
    );
    let position_rent = rent.minimum_balance(
        liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, CLAIM_COUNT)
            .expect("position width"),
    );
    let admission_rent = rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);

    let request = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
        release_set,
        market: shared.core_market.to_bytes(),
        product_record_digest: shared.product.digest,
        product_instance_id: shared.product_id,
        linked_basis_record_digest: shared.linked_basis.digest,
        semantic_basis_id: shared.semantic_basis_id,
        founder: founder.to_bytes(),
        founding_intent_digest: intent_digest,
        aggregate: aggregate.to_bytes(),
        position: position.to_bytes(),
        admission: admission.to_bytes(),
        funding_source: FUNDING_SOURCE.to_bytes(),
        hoard: hoard.to_bytes(),
        custody_replay: custody_replay.to_bytes(),
        rent_credit: rent_credit.to_bytes(),
        rent_program: RENT_PROGRAM_ID.to_bytes(),
        claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
        trading_program: CALLER_PROGRAM_ID.to_bytes(),
        custody_request_digest: LOCK_REQUEST_DIGEST,
        custody_receipt_digest: lock_digest,
        generation: GENERATION,
        claim_count: CLAIM_COUNT,
        quantity: QUANTITY,
        basis_scale: shared.payout_scale,
        pre_source_amount: collateral_atoms,
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: collateral_atoms,
        pre_custody_revision: NORMAL_REPLAY_REVISION - 1,
        post_custody_revision: NORMAL_REPLAY_REVISION,
        aggregate_rent_principal: aggregate_rent,
        position_rent_principal: position_rent,
        admission_rent_principal: admission_rent,
        observed_aggregate_lamports: aggregate_rent,
        observed_position_lamports: position_rent,
        observed_admission_lamports: admission_rent,
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    })
    .expect("canonical founding request");
    let request_bytes = request.to_bytes();
    let request_digest = hash(&request_bytes).to_bytes();

    let permit_bytes =
        SeriesFoundingPermitV1::new(intent, identity(intent_digest), identity(request_digest))
            .expect("core founding permit")
            .encode()
            .expect("permit bytes")
            .to_vec();

    let replay = CustodyReplayV1 {
        caller_role: CallerRoleV1::Trading,
        release_set,
        market: shared.core_market.to_bytes(),
        realm: realm_record.digest,
        context: projected_context,
        caller_program: CALLER_PROGRAM_ID.to_bytes(),
        rent_refund: rent_credit.to_bytes(),
        open_vault_count: 1,
        next_revision: NORMAL_REPLAY_REVISION,
        generation: GENERATION,
        last_request_digest: REALIZE_REQUEST_DIGEST,
        last_poststate_commitment: projected_digest,
    };
    let replay_bytes = replay.to_bytes().expect("custody replay bytes").to_vec();

    let mut instruction_data = Vec::with_capacity(request_bytes.len() + 640);
    instruction_data.extend_from_slice(&request_bytes);
    instruction_data.extend_from_slice(&lock_bytes);
    instruction_data.extend_from_slice(&projected_bytes);

    let caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            release_set,
            shared.core_market.to_bytes(),
            ExecutionRoleV1::Trading,
            intent_digest,
            request_digest,
        )
        .expect("trading caller seeds")
        .as_slices(),
        &CALLER_PROGRAM_ID,
    )
    .0;

    // ---- accounts --------------------------------------------------------
    for record in [
        &shared.product,
        &shared.result_domain,
        &shared.portfolio,
        &shared.linked_basis,
        &realm_record,
    ] {
        add_account(&mut test, record.raw, record.owner, record.bytes.clone());
        add_account(&mut test, record.staging, system_program::ID, Vec::new());
    }
    add_account(
        &mut test,
        shared.core_market,
        CORE_PROGRAM_ID,
        core_state.clone(),
    );
    add_account(&mut test, permit, CORE_PROGRAM_ID, permit_bytes);
    add_account(&mut test, custody_replay, CUSTODY_PROGRAM_ID, replay_bytes);
    add_account(
        &mut test,
        COLLATERAL_MINT,
        token_program_id(),
        collateral_mint_bytes(collateral_atoms, 6),
    );
    add_account(
        &mut test,
        hoard,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, custody_authority, collateral_atoms),
    );
    add_account(
        &mut test,
        rent_credit,
        RENT_PROGRAM_ID,
        LifecycleRentCreditV2::new(
            RefundAuthority::new(REFUND_WALLET).expect("refund authority"),
            LifecycleAccountIdV2::new(shared.core_market.to_bytes()).expect("Market"),
            LifecycleAccountIdV2::new(release_set).expect("release set"),
            GENERATION,
            rent_bump,
        )
        .expect("lifecycle RentCredit")
        .to_bytes()
        .to_vec(),
    );
    // THE FUNDING SOURCE IS A CLOSED ACCOUNT, and the route says so: the Lock
    // consumed it, so it must be System-owned, empty and hold ZERO lamports.
    // That is exactly what an address the bank has never heard of reads as, so
    // it is deliberately NOT planted -- adding a zero-lamport account would be
    // asserting a state the runtime does not keep.
    // Vacant and PREPAID, at exactly the rent the request states it observed.
    for (key, lamports) in [
        (aggregate, aggregate_rent),
        (position, position_rent),
        (admission, admission_rent),
    ] {
        add_account_with_lamports(&mut test, key, system_program::ID, Vec::new(), lamports);
    }
    // The escrow pair rides on BOTH frames and is prepaid only when the record
    // says the Market refunds -- a categorical founding must find it vacant and
    // unfunded, which is the conjunct that makes the shape unforgeable by a
    // caller.
    for (key, lamports) in [
        (escrow_position, position_rent),
        (escrow_admission, admission_rent),
    ] {
        let funded = if shape.seats_escrow() && !matches!(hostile, HostileV1::EscrowRentNotPrepaid)
        {
            lamports
        } else {
            0
        };
        add_account_with_lamports(&mut test, key, system_program::ID, Vec::new(), funded);
    }
    add_account_with_lamports(
        &mut test,
        founder,
        system_program::ID,
        Vec::new(),
        10_000_000_000,
    );
    add_account(&mut test, caller_authority, system_program::ID, Vec::new());

    (
        test,
        FoundingWorld {
            shared,
            core_state,
            activation_cache: activation_cache_key,
            release_set,
            realm_raw: realm_record.raw,
            realm_staging: realm_record.staging,
            permit,
            aggregate,
            position,
            admission,
            escrow_position,
            escrow_admission,
            hoard,
            custody_replay,
            rent_credit,
            request,
            instruction_data,
            caller_authority,
            aggregate_rent,
            position_rent,
            admission_rent,
        },
    )
}

/// The wrapper frame: the Claims program, then the founding's own 33 accounts
/// in the route's own order.
fn founding_instruction(world: &FoundingWorld) -> Instruction {
    let shared = &world.shared;
    let founder = founder_keypair().pubkey();
    let founding = vec![
        AccountMeta::new_readonly(world.caller_authority, false),
        AccountMeta::new_readonly(world.permit, false),
        AccountMeta::new(world.aggregate, false),
        AccountMeta::new(world.position, false),
        AccountMeta::new(world.admission, false),
        AccountMeta::new_readonly(
            Pubkey::new_from_array(world.request.funding_source()),
            false,
        ),
        AccountMeta::new_readonly(world.hoard, false),
        AccountMeta::new_readonly(world.custody_replay, false),
        AccountMeta::new_readonly(shared.linked_basis.raw, false),
        AccountMeta::new_readonly(shared.linked_basis.staging, false),
        AccountMeta::new_readonly(shared.product.raw, false),
        AccountMeta::new_readonly(shared.product.staging, false),
        AccountMeta::new_readonly(shared.result_domain.raw, false),
        AccountMeta::new_readonly(shared.result_domain.staging, false),
        AccountMeta::new_readonly(shared.portfolio.raw, false),
        AccountMeta::new_readonly(shared.portfolio.staging, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(shared.core_market, false),
        AccountMeta::new_readonly(world.activation_cache, false),
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
        AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CORE_PROGRAM_ID), false),
        AccountMeta::new_readonly(CALLER_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CALLER_PROGRAM_ID), false),
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
        AccountMeta::new_readonly(programdata_address(CUSTODY_PROGRAM_ID), false),
        AccountMeta::new_readonly(founder, false),
        AccountMeta::new_readonly(world.rent_credit, false),
        AccountMeta::new_readonly(RENT_PROGRAM_ID, false),
        AccountMeta::new(world.escrow_position, false),
        AccountMeta::new(world.escrow_admission, false),
    ];
    assert_eq!(
        founding.len(),
        CLAIMS_FOUNDING_ACCOUNT_COUNT_V6,
        "the frame is the route's own declared width, read off the route",
    );
    let mut accounts = Vec::with_capacity(1 + founding.len());
    accounts.push(AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false));
    accounts.extend(founding);
    Instruction {
        program_id: CALLER_PROGRAM_ID,
        accounts,
        data: world.instruction_data.clone(),
    }
}

// ---------------------------------------------------------------------------
// Submission
// ---------------------------------------------------------------------------

struct Outcome {
    accepted: bool,
    units: u64,
    refusal: Option<u32>,
    logs: Vec<String>,
}

async fn submit(
    context: &mut ProgramTestContext,
    label: &str,
    instruction: Instruction,
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
        &[&payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .map(ToString::to_string)
        .expect("a submitted transaction carries its own signature");
    let wire_bytes = 1_usize + transaction.signatures.len() * 64 + transaction.message_data().len();
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
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
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
    let refusal = match &processed.result {
        Err(solana_sdk::transaction::TransactionError::InstructionError(
            _,
            solana_sdk::instruction::InstructionError::Custom(code),
        )) => Some(*code),
        _ => None,
    };
    Outcome {
        accepted: processed.result.is_ok(),
        units,
        refusal,
        logs,
    }
}

// ---------------------------------------------------------------------------
// The campaigns
// ---------------------------------------------------------------------------

async fn found(
    shape: FoundingShapeV1,
    hostile: HostileV1,
    label: &str,
) -> (FoundingWorld, ProgramTestContext, Outcome) {
    let (test, world) = world(shape, hostile);
    let mut context = test.start_with_context().await;
    let instruction = founding_instruction(&world);
    let outcome = submit(&mut context, label, instruction).await;
    (world, context, outcome)
}

/// A REFUNDING founding, executed on the real Claims ELF.
///
/// The aggregate, the founder's Position and its admission come into existence,
/// and so do the escrow's Position and admission, because the record says the
/// Market refunds. The founder holds every ordinary coordinate and no failure
/// claim; the escrow holds the failure column and nothing else; the two sum to
/// one complete set at every coordinate.
#[tokio::test]
async fn a_refunding_founding_seats_the_escrow_on_the_real_elf() {
    let (world, mut context, outcome) = found(
        FoundingShapeV1::Refunding,
        HostileV1::None,
        "claims founding: refunding, escrow seated",
    )
    .await;
    assert!(
        outcome.accepted,
        "the refunding founding must be accepted; refusal {:?}, logs {:#?}",
        outcome.refusal, outcome.logs,
    );
    println!(
        "claims founding (refunding, width {CLAIM_COUNT}): accepted, {} CU consumed",
        outcome.units,
    );

    let aggregate = context
        .banks_client
        .get_account(world.aggregate)
        .await
        .expect("aggregate query")
        .expect("the founding created the aggregate");
    assert_eq!(aggregate.owner, CLAIMS_PROGRAM_ID);
    let market = LiabilityBasisMarketViewV2::decode(&aggregate.data).expect("aggregate decodes");
    assert_eq!(market.claim_count, CLAIM_COUNT);
    assert_eq!(market.revision, 1);
    for coordinate in 0..CLAIM_COUNT {
        assert_eq!(
            market.supply(&aggregate.data, coordinate).expect("supply"),
            QUANTITY,
            "a founding issues one complete set: supply is uniform at coordinate {coordinate}",
        );
    }

    let founder_position = context
        .banks_client
        .get_account(world.position)
        .await
        .expect("position query")
        .expect("the founding created the founder Position");
    let founder = LiabilityBasisPositionViewV2::decode(&founder_position.data).expect("decodes");
    let escrow_account = context
        .banks_client
        .get_account(world.escrow_position)
        .await
        .expect("escrow query")
        .expect("a refunding founding creates the escrow Position");
    assert_eq!(escrow_account.owner, CLAIMS_PROGRAM_ID);
    let escrow = LiabilityBasisPositionViewV2::decode(&escrow_account.data).expect("decodes");

    let failure = CLAIM_COUNT - 1;
    for coordinate in 0..CLAIM_COUNT {
        let held = founder
            .balance(&founder_position.data, coordinate)
            .expect("founder balance")
            + escrow
                .balance(&escrow_account.data, coordinate)
                .expect("escrow balance");
        assert_eq!(
            held, QUANTITY,
            "the two Positions sum to one complete set at coordinate {coordinate}",
        );
    }
    assert_eq!(
        founder
            .balance(&founder_position.data, failure)
            .expect("founder failure balance"),
        0,
        "the founder is issued NO failure claim on a refunding Market",
    );
    assert_eq!(
        escrow
            .balance(&escrow_account.data, failure)
            .expect("escrow failure balance"),
        QUANTITY,
        "the escrow holds the whole failure column",
    );
    assert!(
        context
            .banks_client
            .get_account(world.escrow_admission)
            .await
            .expect("escrow admission query")
            .is_some_and(|account| account.owner == CLAIMS_PROGRAM_ID),
        "a refunding founding writes the escrow's ClaimsCapability admission",
    );
}

/// A CATEGORICAL founding over the same thirty-three accounts.
///
/// Only the basis record's payout scale differs. The escrow accounts are in the
/// frame and stay vacant, the founder holds the whole complete set including
/// the last coordinate, and the receipt's post-resource transcript is
/// byte-identical to what three accounts produced before the escrow existed --
/// which is what the V5-to-V6 frame change promised.
#[tokio::test]
async fn a_categorical_founding_leaves_the_escrow_accounts_vacant() {
    let (world, mut context, outcome) = found(
        FoundingShapeV1::Categorical,
        HostileV1::None,
        "claims founding: categorical, no escrow",
    )
    .await;
    assert!(
        outcome.accepted,
        "the categorical founding must be accepted; refusal {:?}, logs {:#?}",
        outcome.refusal, outcome.logs,
    );
    println!(
        "claims founding (categorical, width {CLAIM_COUNT}): accepted, {} CU consumed",
        outcome.units,
    );
    let founder_position = context
        .banks_client
        .get_account(world.position)
        .await
        .expect("position query")
        .expect("the founding created the founder Position");
    let founder = LiabilityBasisPositionViewV2::decode(&founder_position.data).expect("decodes");
    for coordinate in 0..CLAIM_COUNT {
        assert_eq!(
            founder
                .balance(&founder_position.data, coordinate)
                .expect("founder balance"),
            QUANTITY,
            "a categorical founder holds the whole complete set, coordinate {coordinate}",
        );
    }
    let escrow = context
        .banks_client
        .get_account(world.escrow_position)
        .await
        .expect("escrow query");
    assert!(
        escrow.is_none_or(|account| account.owner == system_program::ID && account.data.is_empty()),
        "a categorical founding allocates neither escrow account",
    );
}

/// The fixture's own arithmetic, without a bank.
///
/// Every number the campaigns above submit is derived rather than typed, and
/// this is where that is checkable: the collateral is the exact product, the
/// rents are the widths' own minimums, and the two shapes differ in the payout
/// scale and in nothing else.
#[test]
fn the_two_shapes_differ_in_the_payout_scale_and_nothing_else() {
    let (_, refunding) = world(FoundingShapeV1::Refunding, HostileV1::None);
    let (_, categorical) = world(FoundingShapeV1::Categorical, HostileV1::None);
    assert_eq!(refunding.shared.payout_scale, u64::from(CLAIM_COUNT - 1));
    assert_eq!(categorical.shared.payout_scale, 1);
    assert_eq!(
        refunding.request.claim_count(),
        categorical.request.claim_count(),
    );
    assert_eq!(refunding.request.quantity(), categorical.request.quantity());
    assert_eq!(
        refunding.aggregate_rent, categorical.aggregate_rent,
        "the aggregate's width does not depend on the shape",
    );
    assert_eq!(refunding.position_rent, categorical.position_rent);
    assert_eq!(refunding.admission_rent, categorical.admission_rent);
    assert_eq!(
        refunding.core_state.len(),
        categorical.core_state.len(),
        "and neither does the Core state's",
    );
    assert_eq!(
        refunding.instruction_data.len(),
        categorical.instruction_data.len(),
        "the wire does not move between the two shapes",
    );
    assert_eq!(
        refunding.instruction_data.len(),
        CLAIMS_FOUNDING_REQUEST_BYTES_V5 + 640,
        "request, lock receipt, realization receipt",
    );
    assert_ne!(
        refunding.shared.linked_basis.digest, categorical.shared.linked_basis.digest,
        "the RECORD is what says which shape this Market is",
    );
    assert_eq!(CLAIMS_FOUNDING_RECEIPT_BYTES_V5, 1008);
    let _ = ClaimsFoundingReceiptV5::decode(&[0; CLAIMS_FOUNDING_RECEIPT_BYTES_V5]).is_err();
}

// ---------------------------------------------------------------------------
// The two conjuncts CLAIMS-17 added, which had never run anywhere
// ---------------------------------------------------------------------------

/// A refunding founding whose escrow is not the MARKET's own refuses `0x5010`.
///
/// `FailureEscrowIdentityV1::derive` became the sole author of the escrow's
/// identity for both the founding that seats it and the complete-set gate that
/// requires it to stay seated, and the point of one author is that the same
/// mistake gets the same code wherever it is made. The account this campaign
/// substitutes is a well-formed, vacant Position of the SAME aggregate under a
/// different owner, so every seed helper succeeds and only the derivation
/// disagrees -- which is the only way to reach the conjunct rather than a
/// vacancy check in front of it.
#[tokio::test]
async fn a_refunding_founding_whose_escrow_is_not_the_markets_own_refuses() {
    let (_, _, outcome) = found(
        FoundingShapeV1::Refunding,
        HostileV1::EscrowIsNotTheMarketsOwn,
        "claims founding: refunding, escrow substituted",
    )
    .await;
    assert!(!outcome.accepted);
    assert_eq!(
        outcome.refusal,
        Some(dclutch_claims_sbf::ClaimsSbfError::FailureEscrow as u32),
        "a founding whose escrow account is not the derived one is the routed \
         split's mistake made one stage earlier, and carries the same code; \
         logs {:#?}",
        outcome.logs,
    );
}

/// A refunding founding whose escrow rent is not prepaid refuses `0x5186 Rent`.
///
/// The escrow's two accounts ride on EVERY founding and are prepaid on exactly
/// the refunding ones, which is what stops a caller signalling a Market's shape
/// by what it funds. This campaign leaves them at zero lamports -- the state a
/// categorical founding is entitled to -- over a record that says the Market
/// refunds, and the founding refuses rather than seating an escrow the founder
/// did not pay for.
#[tokio::test]
async fn a_refunding_founding_whose_escrow_rent_is_not_prepaid_refuses() {
    let (_, _, outcome) = found(
        FoundingShapeV1::Refunding,
        HostileV1::EscrowRentNotPrepaid,
        "claims founding: refunding, escrow rent absent",
    )
    .await;
    assert!(!outcome.accepted);
    assert_eq!(
        outcome.refusal,
        Some(FOUNDING_RENT_REFUSAL_V5),
        "the escrow rent conjunct is the one that must fire; logs {:#?}",
        outcome.logs,
    );
    assert_ne!(
        FOUNDING_RENT_REFUSAL_V5,
        dclutch_claims_sbf::ClaimsSbfError::FailureEscrow as u32,
        "the two escrow-seating refusals must be distinguishable, or this pair \
         and the one above prove one thing between them",
    );
}

/// `ClaimsFoundingSbfErrorV5::Rent`, derived from the registered band rather
/// than typed.
///
/// The founding enum is private to the Claims program, so the discriminant
/// cannot be read off it from here. It is derived instead from the two things
/// that fix it -- the program's registered refusal base and the founding
/// route's own sub-band offset -- so a band move breaks this line rather than
/// silently re-pointing it at another route's code. `0x5186` is the seventh
/// variant of the run that starts at `CLAIMS_REFUSAL_BASE + 0x180`.
const FOUNDING_RENT_REFUSAL_V5: u32 = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x180 + 6;
