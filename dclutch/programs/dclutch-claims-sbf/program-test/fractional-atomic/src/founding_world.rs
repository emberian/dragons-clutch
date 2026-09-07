//! The founding world: the first Claims founding executed against a real ELF,
//! as one author for every campaign that needs a FOUNDED Market.
//!
//! Extracted from `tests/claims_founding.rs` (whose head carries the full
//! account of what had to exist first, the forged prestate and its digest
//! chain, and the per-stage CU table) so that the conservation campaign can
//! split and merge on a Market the founding route created rather than on one
//! a fixture planted. Nothing here was rewritten in the move; the founding
//! tests pass through it unchanged.

// The fixture's constants and fields are documented by the founding campaign's
// head, which this module was cut from; a doc line on each of forty planted
// coordinates would restate that head forty times.
#![allow(missing_docs)]

use std::{env, fs, path::PathBuf};

use crate::{
    campaign_support::{
        ReleaseSetInputV1, activation_cache, add_account, add_account_with_lamports,
        add_upgradeable_program, collateral_mint_bytes, finalized, programdata_address,
        token_account_bytes_for, token_program_id,
    },
    narrow_fixture::{
        NarrowBasisInputV3, NarrowFixtureInputV2, NarrowFixtureV2, compile_narrow_fixture_v3,
    },
};
use dclutch_claims::{
    founder_bond_v1::founding_bond_size_v1,
    founding_v5::{
        CLAIMS_FOUNDING_ACCOUNT_COUNT_V6, ClaimsFoundingAggregateSeedsV5,
        ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5,
    },
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        liability_basis_vector_width_v2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionSeedsV2,
    },
};
use dclutch_custody::token_svm::{PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID};
use dclutch_custody::{
    CallerRoleV1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyVaultSeedsV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCustodyLockReceiptV1,
    ProjectedCustodyReceiptV1,
};
use dclutch_market::capability_manifest::funding::derive_funded_rent_rate_v2;
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_market::rent::RefundAuthority;
use dclutch_market::rent::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
};
use dclutch_market::{
    CoreState, FoundingIntentV5, Identity, Phase, Readiness, SeriesFoundingPermitV1,
};
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
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

pub const CLAIMS_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa1; 32]);
pub const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa2; 32]);
pub const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa3; 32]);
pub const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa4; 32]);
/// The Trading role, which for this campaign is the founding caller.
pub const CALLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xa9; 32]);
pub const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0xb6; 32]);
pub const COLLATERAL_MINT: Pubkey = Pubkey::new_from_array([0x74; 32]);
pub const FUNDING_SOURCE: Pubkey = Pubkey::new_from_array([0x66; 32]);
pub const REFUND_WALLET: [u8; 32] = [0x5f; 32];
pub const RENT_BENEFICIARY: Pubkey = Pubkey::new_from_array([0x43; 32]);

pub const CLAIM_COUNT: u32 = 4;
pub const GENERATION: u64 = 41;
pub const CUSTODY_CONTEXT: [u8; 32] = [0x62; 32];
pub const TICKET_CONTEXT: [u8; 32] = [0x71; 32];
pub const PARENT_ROOT: [u8; 32] = [0x72; 32];
pub const SERIES_SOURCE: [u8; 32] = [0x73; 32];
/// The digest of the Realize request Custody consumed. It only has to be
/// nonzero and DIFFERENT from the Lock request's, because the intent carries
/// the projected pair and the request the realized one.
pub const REALIZE_REQUEST_DIGEST: [u8; 32] = [0x81; 32];
pub const LOCK_REQUEST_DIGEST: [u8; 32] = [0x83; 32];
pub const EXPIRY_SLOT: u64 = 10_000;
/// Complete sets the founding issues.
pub const QUANTITY: u64 = 7;
/// The projected replay revision the Lock stepped to.
pub const PROJECTED_RESULTING_REVISION: u64 = 3;
/// PINNED TO ONE BY THE INTENT ITSELF. `validate_coordinates` refuses any other
/// value, which in turn pins the request's post-custody revision to one and its
/// pre-custody revision to zero -- a founding's Custody cursor has taken exactly
/// one step, the Realize, and there is no earlier normal act to have taken
/// another.
pub const NORMAL_REPLAY_REVISION: u64 = 1;

pub fn founder_keypair() -> Keypair {
    Keypair::new_from_array([0x31; 32])
}

// ---------------------------------------------------------------------------
// Artifacts
// ---------------------------------------------------------------------------

pub struct Artifacts {
    pub claims: Vec<u8>,
    pub registry: Vec<u8>,
    pub core: Vec<u8>,
    pub custody: Vec<u8>,
    pub rent: Vec<u8>,
    pub caller: Vec<u8>,
}

pub fn artifacts() -> Artifacts {
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
pub enum FoundingShapeV1 {
    /// Payout scale 1: a categorical Market, which seats no escrow.
    Categorical,
    /// Payout scale `basis_width - 1`: a refunding Market, which seats one.
    Refunding,
}

impl FoundingShapeV1 {
    pub fn basis(self) -> NarrowBasisInputV3<'static> {
        match self {
            Self::Categorical => NarrowBasisInputV3::Categorical,
            Self::Refunding => NarrowBasisInputV3::CategoricalRefunding,
        }
    }

    pub fn seats_escrow(self) -> bool {
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
pub enum HostileV1 {
    /// Nothing: the founding this campaign expects to be accepted.
    None,
    /// The escrow Position is a Position of the same aggregate, owned by
    /// somebody else. Every seed helper still succeeds; only the derivation
    /// disagrees.
    EscrowIsNotTheMarketsOwn,
    /// The escrow's accounts are the Market's own and are left UNFUNDED, as a
    /// categorical founding may leave them.
    EscrowRentNotPrepaid,
    /// The escrow Position holds its rent and all but the LAST lamport of the
    /// founder bond. Every other account, and the admission beside it, is the
    /// accepted world's.
    FounderBondOneLamportShort,
    /// The escrow Position holds exactly its own rent and no bond: the state a
    /// host that priced the escrow by `Rent::minimum_balance` alone produces.
    FounderBondAbsent,
}

pub struct FoundingWorld {
    pub shared: NarrowFixtureV2,
    pub core_state: Vec<u8>,
    pub activation_cache: Pubkey,
    pub release_set: [u8; 32],
    pub realm_raw: Pubkey,
    pub realm_staging: Pubkey,
    pub permit: Pubkey,
    pub aggregate: Pubkey,
    pub position: Pubkey,
    pub admission: Pubkey,
    pub escrow_position: Pubkey,
    pub escrow_admission: Pubkey,
    pub hoard: Pubkey,
    pub custody_replay: Pubkey,
    pub rent_credit: Pubkey,
    pub request: ClaimsFoundingRequestV5,
    pub instruction_data: Vec<u8>,
    pub caller_authority: Pubkey,
    pub aggregate_rent: u64,
    pub position_rent: u64,
    pub admission_rent: u64,
    /// The founder bond this Market's width and rent rate require the escrow
    /// Position to hold ON TOP of its own recorded rent (decision 0033). Zero
    /// is not a value this takes: the size rule's floor is what MANDATORY
    /// means.
    pub bond: u64,
}

/// The founding world with no collateral beyond the founding's own.
pub fn world(shape: FoundingShapeV1, hostile: HostileV1) -> (ProgramTest, FoundingWorld) {
    world_with_extra_collateral(shape, hostile, 0)
}

/// The founding world with `extra_collateral` more atoms minted, for a
/// campaign that seats a second collateral holder beside the founder.
pub fn world_with_extra_collateral(
    shape: FoundingShapeV1,
    hostile: HostileV1,
    extra_collateral: u64,
) -> (ProgramTest, FoundingWorld) {
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
        &[dclutch_registry::ACTIVATION_PDA_DOMAIN_V1, &release_set],
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
        HostileV1::None
        | HostileV1::EscrowRentNotPrepaid
        | HostileV1::FounderBondOneLamportShort
        | HostileV1::FounderBondAbsent => escrow_owner,
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
    let position_width =
        liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, CLAIM_COUNT)
            .expect("position width");
    let position_rent = rent.minimum_balance(position_width);
    let admission_rent = rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    // The founder bond (decision 0033), derived here the way
    // `authenticate_founder_bond` derives it and not one step differently: the
    // rate comes from two readings of the SAME `Rent` the escrow's own rent
    // came from, and the size rule is evaluated at this Market's width with a
    // zero ladder term. A fixture that typed the bond as a number would pass
    // whatever the route computed; this one refuses to agree by accident.
    let bond = founding_bond_size_v1(
        derive_funded_rent_rate_v2(rent.minimum_balance(0), position_width, position_rent)
            .expect("affine rent"),
        CLAIM_COUNT,
        0,
    )
    .expect("size rule")
    .bond;

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
        collateral_mint_bytes(collateral_atoms + extra_collateral, 6),
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
    // The Position's share of that prepayment is its rent PLUS the founder
    // bond (decision 0033); the two bond hostiles move that one number by one
    // lamport and by the whole bond, and touch nothing else in the world.
    let escrow_position_funded = if shape.seats_escrow() {
        match hostile {
            HostileV1::None | HostileV1::EscrowIsNotTheMarketsOwn => position_rent + bond,
            HostileV1::FounderBondOneLamportShort => position_rent + bond - 1,
            HostileV1::FounderBondAbsent => position_rent,
            HostileV1::EscrowRentNotPrepaid => 0,
        }
    } else {
        0
    };
    let escrow_admission_funded =
        if shape.seats_escrow() && !matches!(hostile, HostileV1::EscrowRentNotPrepaid) {
            admission_rent
        } else {
            0
        };
    for (key, lamports) in [
        (escrow_position, escrow_position_funded),
        (escrow_admission, escrow_admission_funded),
    ] {
        add_account_with_lamports(&mut test, key, system_program::ID, Vec::new(), lamports);
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
            bond,
        },
    )
}

/// The wrapper frame: the Claims program, then the founding's own 33 accounts
/// in the route's own order.
pub fn founding_instruction(world: &FoundingWorld) -> Instruction {
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

pub struct Outcome {
    pub accepted: bool,
    pub units: u64,
    pub refusal: Option<u32>,
    pub logs: Vec<String>,
    /// Signatures the transaction carried, for L7.
    pub signature_count: usize,
    /// The fee the bank quoted for the message, when it would; L7's declared side.
    pub fee_lamports: Option<u64>,
}

pub async fn submit(
    context: &mut ProgramTestContext,
    label: &str,
    instruction: Instruction,
) -> Outcome {
    let payer = context.payer.insecure_clone();
    submit_with(context, label, &[instruction], &payer, &[]).await
}

/// Submit one transaction paid and signed by `payer`, with `extra_signers`
/// co-signing, and record its evidence under `label`.
///
/// The one submission path every campaign in this crate uses, so a fee, a
/// signature count and a refusal are read the same way everywhere.
pub async fn submit_with(
    context: &mut ProgramTestContext,
    label: &str,
    instructions: &[Instruction],
    payer: &Keypair,
    extra_signers: &[&Keypair],
) -> Outcome {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let mut signers: Vec<&Keypair> = Vec::with_capacity(extra_signers.len() + 1);
    signers.push(payer);
    signers.extend_from_slice(extra_signers);
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &signers,
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .map(ToString::to_string)
        .expect("a submitted transaction carries its own signature");
    let wire_bytes = 1_usize + transaction.signatures.len() * 64 + transaction.message_data().len();
    let signature_count = transaction.signatures.len();
    let fee_lamports = context
        .banks_client
        .get_fee_for_message(transaction.message.clone())
        .await
        .ok()
        .flatten();
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
        signature_count,
        fee_lamports,
    }
}

pub async fn found(
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
