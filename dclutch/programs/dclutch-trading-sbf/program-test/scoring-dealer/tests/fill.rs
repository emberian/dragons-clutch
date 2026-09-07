//! Real-ELF evidence for the scoring Dealer's `DealerFill` (`DCLSFLR1`).
//!
//! WHY THIS EXISTS. The dealer family landed with no program test and no CU
//! measurement of any kind, and that is how `process_dealer_fill_v1` reached
//! `main` with a 4,288-byte stack frame against SBPF v0's 4,096: the backend
//! said so on every build, `cargo build-sbf` exited zero, and nothing else ever
//! executed the route. A frame that overruns writes over its own locals, so the
//! route could not have run correctly -- and nothing in the tree could have
//! shown that, before or after a repair.
//!
//! WHAT IT DRIVES. One `DealerFill` transaction against the real
//! `dclutch_trading_sbf` ELF, over the exact 19-account prefix and the 64
//! accounts of the four windows the Lean's `fillFrame` states, and one hostile
//! per authentication concern the route owns. Each hostile asserts the exact
//! `ScoringDealerErrorV1` code the runtime returned, read out of the runtime's
//! own logs -- never out of what this campaign expected.
//!
//! WHAT IT DOES NOT DRIVE, said out loud. The accepted fill here is the NULL
//! fill: `mint = 0`, `receive = deliver = 0`. It runs the whole route --
//! `parse_prefix`, all five authenticators, the Claims aggregate and both
//! Positions, the LMSR kernel (`admit_fill`), `cashLegs`, the window identity
//! join, the fund commit and the receipt -- and it is the ONLY fill shape that
//! reaches the end without a CPI, because every nonzero coordinate produces
//! either a Claims signed delta or a Custody transfer leg. A fill that actually
//! moves a claim or an atom needs the founded Claims aggregate, the admitted
//! Positions, the opened Custody vaults and the SPL mint that only a full
//! Claims + Custody fixture can supply; that fixture is not written and this
//! campaign does not pretend to it. What is pinned here is that the re-shaped
//! route executes on chain, refuses its hostiles by name, and costs what it
//! costs.

use std::vec::Vec;

use dclutch_claims::liability_basis_state_v2::{
    LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
    encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
};
use dclutch_claims::protocol_position_v2::ProtocolPositionSeedsV2;
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CallerRoleV1, CompartmentV1, CustodyReplayV1, CustodyVaultSeedsV1, TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_market::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
    STATE_BYTES, StateBumpsV1,
};
use dclutch_program_test_evidence::TransactionEvidence;
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    ProgramIdentityV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use dclutch_trading::scoring_rule::records_v1::{DealerFundV1, FundPhaseV1, ScoringRuleRecordV1};
use dclutch_trading::scoring_rule::requests_v1::{
    DealerFillRequestV1, DealerReceiptV1, DealerRouteV1, FILL_FRAME_ACCOUNTS,
};
use dclutch_trading::scoring_rule::{
    RuleParameters, Vector, ZERO_VECTOR, potential, prices_of, subsidy_of,
};
use dclutch_trading_sbf::scoring_dealer_v1::fill::FILL_WINDOW_ACCOUNTS;
use solana_account::Account;
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext, tokio};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::bpf_loader_upgradeable;
use solana_transaction::versioned::VersionedTransaction;

/// The ELF this campaign loads, by the name `cargo build-sbf` gives it.
const TRADING_ELF: &str = "dclutch_trading_sbf";
/// `K`: the rule's ordinary outcome count.
const OUTCOME_COUNT: u8 = 2;
/// The aggregate's runtime claim count: `K` ordinary plus decision 0025's
/// failure coordinate.
const CLAIM_COUNT: u32 = 3;
/// The fund revision the accepted request names.
const FUND_REVISION: u64 = 7;
/// Atoms per claim unit.
const CLAIM_UNIT_ATOMS: u64 = 1_000;
/// Solana's packet maximum; ProgramTest submits no packet, so the campaign
/// measures against it rather than asking the runtime to enforce it.
const PACKET_DATA_BYTES: usize = 1_232;

/// The claims window: the fixed signed-delta frame plus the two Positions.
const CLAIMS_WINDOW_ACCOUNTS: usize =
    dclutch_claims::frame_spec_v1::SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 as usize + 2;

/// One substitution the route must catch, and the concern it belongs to.
///
/// Each variant moves exactly one thing out of the accepted fixture, so the
/// code the runtime returns names the authenticator that caught it rather than
/// whichever check happens to run first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostileV1 {
    /// Nothing moved: the accepted null fill.
    None,
    /// The wire carries the route's magic and the wrong width.
    Wire,
    /// The frame is one account short of the four windows.
    FrameWidth,
    /// The signer at coordinate zero is not the request's taker.
    TakerSubstituted,
    /// The fund account is owned by another program.
    FundOwner,
    /// The request names a revision the fund has left.
    StaleRevision,
    /// The request's `K` disagrees with the fund's.
    WidthDisagreement,
    /// The rule body is not the one the fund sealed.
    RuleUnsealed,
    /// The Market has not opened, and the fill admits only Open.
    MarketNotOpen,
    /// The frame's Claims program is not the one the release activated.
    ClaimsSubstituted,
    /// The vault is not the one the fund records.
    VaultSubstituted,
    /// The Dealer's Position is not at the PDA its aggregate and fund derive.
    PositionSubstituted,
    /// The taker's Position is owned by somebody else.
    TakerPositionOwner,
    /// The price vector is off the Dealer's own schedule.
    OffSchedule,
    /// The Dealer's inventory holds a complete set, so R1 refuses.
    NotNormalized,
}

/// The keys and the instruction one fixture produced.
struct World {
    /// The taker: the frame's sole signer, and a second signature on the wire.
    taker: Keypair,
    /// The fund PDA the route advances.
    fund: Pubkey,
    /// The one instruction the campaign submits.
    instruction: Instruction,
}

fn key(tag: u8) -> Pubkey {
    Pubkey::new_from_array([tag; 32])
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn release(program: Pubkey, semantic: u8) -> ArtifactReleaseV1 {
    ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("program identity"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader identity"),
        programdata(program).to_bytes(),
        ContentId::new([semantic; 32]).expect("semantic release"),
        [semantic.wrapping_add(1); 32],
        0,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("immutable artifact release")
}

fn artifact_id(value: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
    ArtifactReleaseIdV1::new(hash(&value.to_bytes()).to_bytes()).expect("artifact identity")
}

fn binding(value: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
    ExecutionRoleBindingV1::new(value.program(), artifact_id(value))
}

fn activation_input(value: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
    ArtifactActivationInputV1::new(
        artifact_id(value),
        value,
        DeploymentObservationV1::new(
            value.program().to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            value.programdata(),
            bpf_loader_upgradeable::ID.to_bytes(),
            value.deployment_slot(),
            value.elf_digest(),
            value.upgrade_authority(),
        )
        .expect("current immutable deployment observation"),
    )
}

/// The release set this Market selected, and the activation cache at its PDA.
///
/// The fill reads the cache for exactly two facts -- that THIS program is the
/// release's Trading role, and which program is its Claims role -- so the cache
/// is built the way the Registry builds one and nothing further is staged.
fn activated(
    core: Pubkey,
    claims: Pubkey,
    trading: Pubkey,
    custody: Pubkey,
    registry: Pubkey,
) -> ([u8; 32], Pubkey, Vec<u8>) {
    let core_release = release(core, 0x31);
    let claims_release = release(claims, 0x32);
    let trading_release = release(trading, 0x33);
    let custody_release = release(custody, 0x34);
    let release_set = ExecutionReleaseSetV1::new(
        binding(core_release),
        binding(claims_release),
        binding(trading_release),
        binding(core_release),
        binding(custody_release),
    )
    .expect("complete execution release set");
    let release_set_id = hash(&release_set.to_bytes()).to_bytes();
    let content = ContentId::new(release_set_id).expect("release set identity");
    let mut cache = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
    initialize_activation_cache_v1(&mut cache, content).expect("activation cache");
    for (role, selected) in [
        (ExecutionRoleV1::Core, core_release),
        (ExecutionRoleV1::Claims, claims_release),
        (ExecutionRoleV1::Trading, trading_release),
        (ExecutionRoleV1::Resolution, core_release),
        (ExecutionRoleV1::Custody, custody_release),
    ] {
        activate_execution_role_into_v1(
            &mut cache,
            content,
            &release_set,
            role,
            &activation_input(selected),
        )
        .expect("activate exact role");
    }
    let address =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set_id], &registry).0;
    (release_set_id, address, cache)
}

/// One live Core Market at its own PDA.
///
/// The seed projection excludes `market_id`, which is what makes this
/// constructible: derive from the other eight coordinates, then write the
/// derived address back as the ninth.
fn core_market(
    core: Pubkey,
    registry: Pubkey,
    release_set: [u8; 32],
    open: bool,
) -> (Pubkey, Vec<u8>) {
    let identity = |tag: u8| CoreIdentity::new([tag; 32]).expect("nonzero core identity");
    let mut market = MarketIdentity {
        market_id: identity(0xa1),
        realm_id: identity(0xa2),
        product_record: identity(0xa3),
        product_id: identity(0xa4),
        resolution_policy: identity(0xa5),
        capability_manifest: identity(0xa6),
        selected_release_set: CoreIdentity::new(release_set).expect("nonzero release set"),
        registry_program: CoreIdentity::new(registry.to_bytes()).expect("nonzero registry"),
        generation: 9,
    };
    let address =
        Pubkey::find_program_address(&MarketCoreStateSeedsV2::new(market).as_slices(), &core).0;
    market.market_id = CoreIdentity::new(address.to_bytes()).expect("derived market id");
    // A Market before it opens is the only `valid_static` shape that is not
    // Open and needs no terminal receipt (`CoreState::valid_static`).
    let body = CoreState {
        phase: if open { Phase::Open } else { Phase::Founding },
        readiness: if open {
            Readiness::Consumed
        } else {
            Readiness::Ready
        },
        terminal_winner: 0,
        identity: market,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity(0xa9),
        terminal_receipt: None,
        bumps: StateBumpsV1::UNRECORDED,
    }
    .encode()
    .expect("canonical Market state");
    assert_eq!(body.len(), STATE_BYTES, "the Market body is the chain's");
    (address, body.to_vec())
}

fn rule_parameters() -> RuleParameters {
    RuleParameters {
        outcome_count: OUTCOME_COUNT,
        liquidity: 1_000,
        scale: 1_000_000,
        tolerance: 1,
    }
    .admit()
    .expect("admitted rule parameters")
}

fn vector(values: &[u64]) -> Vector {
    let mut output = ZERO_VECTOR;
    for (slot, value) in output.iter_mut().zip(values) {
        *slot = *value;
    }
    output
}

fn add(test: &mut ProgramTest, address: Pubkey, owner: Pubkey, data: Vec<u8>) {
    test.add_account(
        address,
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    );
}

/// The whole fixture, and the one instruction it exists to submit.
#[expect(
    clippy::too_many_lines,
    reason = "one fixture, stated in the order the route reads it; splitting it would hide the join"
)]
fn world(hostile: HostileV1) -> (ProgramTest, World) {
    let trading = key(0x77);
    let core = key(0xc0);
    let claims = key(0xc1);
    let custody = key(0xcd);
    let registry = key(0x8e);
    let impostor = key(0x1f);

    let (release_set, cache_address, cache) = activated(core, claims, trading, custody, registry);
    let (market, market_body) = core_market(
        core,
        registry,
        release_set,
        hostile != HostileV1::MarketNotOpen,
    );
    let market_id = market.to_bytes();
    let dealer_id = [0xd1_u8; 32];

    let (fund, fund_bump) =
        Pubkey::find_program_address(&DealerFundV1::seeds(&market_id, &dealer_id), &trading);
    let vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            market_id,
            release_set,
            fund.to_bytes(),
            CompartmentV1::TradingPrincipal,
        )
        .as_slices(),
        &custody,
    )
    .0;
    let parameters = rule_parameters();
    let rule_record = ScoringRuleRecordV1 {
        parameters,
        market: market_id,
        dealer_id,
        subsidy: subsidy_of(parameters).expect("subsidy"),
    };
    let rule_body = rule_record.to_bytes().expect("rule record").to_vec();
    let rule = Pubkey::find_program_address(
        &ScoringRuleRecordV1::seeds(&market_id, &dealer_id),
        &trading,
    )
    .0;

    // The Dealer's inventory, and the potential the fund carries for it. A
    // complete set (R1's refusal) is one substitution away from the admitted
    // all-zero inventory, and nothing else about the fixture moves with it.
    let inventory = if hostile == HostileV1::NotNormalized {
        vector(&[1, 1])
    } else {
        ZERO_VECTOR
    };
    let carried = potential(parameters, &inventory).expect("potential");
    let fund_record = DealerFundV1 {
        outcome_count: OUTCOME_COUNT,
        phase: FundPhaseV1::Open,
        market: market_id,
        dealer_id,
        sponsor: [0x5b; 32],
        rule_digest: hash(&rule_body).to_bytes(),
        vault: vault.to_bytes(),
        claim_unit_atoms: CLAIM_UNIT_ATOMS,
        cash: 5_000_000,
        inventory_minimum: carried.minimum,
        liquidity_cost: carried.cost,
        revision: FUND_REVISION,
        bump: fund_bump,
    };

    let taker = Keypair::new();
    let aggregate = key(0xa0);
    let basis_id = [0xb1_u8; 32];
    let mut aggregate_body = vec![0_u8; 256 + 8 * CLAIM_COUNT as usize];
    encode_liability_basis_market_into_v2(
        LiabilityBasisMarketInputV2 {
            revision: 4,
            logical_market: market_id,
            release_set,
            registry_program: registry.to_bytes(),
            product_instance_id: [0xa4; 32],
            basis_id,
            realm_id: [0xa2; 32],
            custody_context: fund.to_bytes(),
            generation: 9,
        },
        &[100, 100, 0],
        &mut aggregate_body,
    )
    .expect("aggregate body");

    let dealer_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), fund.to_bytes())
            .expect("Position coordinates")
            .as_slices(),
        &claims,
    )
    .0;
    let position_body = |owner: [u8; 32], balances: &[u64]| {
        let mut body = vec![0_u8; 128 + 8 * CLAIM_COUNT as usize];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 3,
                market_account: aggregate.to_bytes(),
                owner,
                basis_id,
            },
            balances,
            &mut body,
        )
        .expect("Position body");
        body
    };
    let taker_position = key(0xa7);
    let taker_position_owner = if hostile == HostileV1::TakerPositionOwner {
        impostor.to_bytes()
    } else {
        taker.pubkey().to_bytes()
    };

    let replay = key(0x9a);
    let replay_body = CustodyReplayV1 {
        caller_role: CallerRoleV1::Trading,
        release_set,
        market: market_id,
        realm: [0xa2; 32],
        context: fund.to_bytes(),
        caller_program: trading.to_bytes(),
        rent_refund: [0x5b; 32],
        open_vault_count: 2,
        next_revision: 4,
        generation: 9,
        last_request_digest: [0x11; 32],
        last_poststate_commitment: [0x22; 32],
    }
    .to_bytes()
    .expect("replay body")
    .to_vec();

    let mut test = ProgramTest::default();
    test.add_upgradeable_program_to_genesis(TRADING_ELF, &trading);
    // The Registry is read for its identity and its executable bit and is never
    // invoked; the Trading ELF standing at that address is the cheapest
    // genuinely-executable account this fixture can offer.
    test.add_upgradeable_program_to_genesis(TRADING_ELF, &registry);
    add(&mut test, cache_address, registry, cache);
    add(&mut test, market, core, market_body);
    add(
        &mut test,
        fund,
        if hostile == HostileV1::FundOwner {
            impostor
        } else {
            trading
        },
        fund_record.to_bytes().expect("fund record").to_vec(),
    );
    add(
        &mut test,
        rule,
        trading,
        if hostile == HostileV1::RuleUnsealed {
            // A rule that is canonical, is at this PDA, and is not the body the
            // fund sealed: only `rule_digest` can catch it.
            ScoringRuleRecordV1 {
                parameters: RuleParameters {
                    liquidity: 2_000,
                    ..parameters
                }
                .admit()
                .expect("second admitted rule"),
                ..rule_record
            }
            .to_bytes()
            .expect("substituted rule")
            .to_vec()
        } else {
            rule_body
        },
    );
    add(&mut test, aggregate, claims, aggregate_body);
    let mut dealer_balances = vec![0_u64; CLAIM_COUNT as usize];
    for (slot, value) in dealer_balances.iter_mut().zip(inventory.iter()) {
        *slot = *value;
    }
    add(
        &mut test,
        dealer_position,
        claims,
        position_body(fund.to_bytes(), &dealer_balances),
    );
    add(
        &mut test,
        taker_position,
        claims,
        position_body(taker_position_owner, &[0, 0, 0]),
    );
    add(&mut test, replay, custody, replay_body);
    add(
        &mut test,
        taker.pubkey(),
        solana_sdk_ids::system_program::ID,
        Vec::new(),
    );
    // The window slots this route never reads still have to be accounts.
    let filler = |tag: u8| key(tag);
    for tag in 0xe0..0xea_u8 {
        add(&mut test, filler(tag), custody, vec![tag; 32]);
    }

    // The prices the request carries: the Dealer's own schedule at the
    // post-fill inventory, which for a null fill is the inventory itself.
    let schedule = prices_of(parameters, &inventory).expect("schedule");
    let prices = if hostile == HostileV1::OffSchedule {
        let mut moved = schedule;
        let (first, second) = (moved[0], moved[1]);
        moved[0] = first + parameters.tolerance + 1;
        moved[1] = second - parameters.tolerance - 1;
        moved
    } else {
        schedule
    };
    let request = DealerFillRequestV1 {
        outcome_count: if hostile == HostileV1::WidthDisagreement {
            OUTCOME_COUNT + 1
        } else {
            OUTCOME_COUNT
        },
        market: market_id,
        dealer_id,
        taker: if hostile == HostileV1::TakerSubstituted {
            impostor.to_bytes()
        } else {
            taker.pubkey().to_bytes()
        },
        expected_fund_revision: if hostile == HostileV1::StaleRevision {
            FUND_REVISION + 1
        } else {
            FUND_REVISION
        },
        mint: 0,
        prices,
        receive: ZERO_VECTOR,
        deliver: ZERO_VECTOR,
    };
    let mut data = request.to_bytes().expect("fill request").to_vec();
    if hostile == HostileV1::Wire {
        data.truncate(data.len() - 1);
    }

    let mut accounts = vec![
        AccountMeta::new(taker.pubkey(), true),
        AccountMeta::new(fund, false),
        AccountMeta::new_readonly(rule, false),
        AccountMeta::new_readonly(market, false),
        AccountMeta::new(aggregate, false),
        AccountMeta::new(
            if hostile == HostileV1::PositionSubstituted {
                taker_position
            } else {
                dealer_position
            },
            false,
        ),
        AccountMeta::new(
            if hostile == HostileV1::PositionSubstituted {
                dealer_position
            } else {
                taker_position
            },
            false,
        ),
        AccountMeta::new_readonly(filler(0xe0), false),
        AccountMeta::new_readonly(
            if hostile == HostileV1::ClaimsSubstituted {
                impostor
            } else {
                claims
            },
            false,
        ),
        AccountMeta::new(filler(0xe1), false),
        AccountMeta::new(
            if hostile == HostileV1::VaultSubstituted {
                filler(0xe2)
            } else {
                vault
            },
            false,
        ),
        AccountMeta::new(filler(0xe3), false),
        AccountMeta::new(filler(0xe4), false),
        AccountMeta::new_readonly(filler(0xe5), false),
        AccountMeta::new_readonly(custody, false),
        AccountMeta::new_readonly(filler(0xe6), false),
        AccountMeta::new_readonly(filler(0xe7), false),
        AccountMeta::new_readonly(cache_address, false),
        AccountMeta::new_readonly(registry, false),
    ];
    assert_eq!(
        accounts.len(),
        FILL_FRAME_ACCOUNTS,
        "the prefix is the Lean's"
    );

    // The Claims window. Only four coordinates are read before the null fill's
    // early return: the aggregate and the two Positions, which must be the SAME
    // accounts the prefix named, and two the route hashes for their bytes.
    let mut claims_window = vec![filler(0xe8); CLAIMS_WINDOW_ACCOUNTS];
    claims_window[1] = aggregate;
    claims_window[2] = taker_position;
    claims_window[4] = aggregate;
    claims_window[CLAIMS_WINDOW_ACCOUNTS - 2] = if hostile == HostileV1::PositionSubstituted {
        taker_position
    } else {
        dealer_position
    };
    claims_window[CLAIMS_WINDOW_ACCOUNTS - 1] = if hostile == HostileV1::PositionSubstituted {
        dealer_position
    } else {
        taker_position
    };
    for address in claims_window {
        accounts.push(AccountMeta::new(address, false));
    }
    // Three Custody Transfer windows. Coordinate 8 of the first is the replay
    // the fund's leg reads before Custody is told the amount is zero.
    for window in 0..3_usize {
        for coordinate in 0..TRANSFER_ACCOUNT_COUNT_V1 as usize {
            let address = if window == 0 && coordinate == 8 {
                replay
            } else {
                filler(0xe9)
            };
            accounts.push(AccountMeta::new(address, false));
        }
    }
    assert_eq!(
        accounts.len(),
        FILL_FRAME_ACCOUNTS + FILL_WINDOW_ACCOUNTS,
        "the four windows are the Lean's"
    );
    if hostile == HostileV1::FrameWidth {
        accounts.pop();
    }

    (
        test,
        World {
            taker,
            fund,
            instruction: Instruction {
                program_id: trading,
                accounts,
                data,
            },
        },
    )
}

fn lookup_addresses(payer: Pubkey, taker: Pubkey, instruction: &Instruction) -> Vec<Pubkey> {
    let mut addresses = Vec::new();
    for address in std::iter::once(instruction.program_id)
        .chain(instruction.accounts.iter().map(|meta| meta.pubkey))
    {
        if address != payer && address != taker && !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    addresses
}

async fn create_live_lookup_table(
    context: &mut ProgramTestContext,
    addresses: &[Pubkey],
) -> Pubkey {
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    context
        .warp_to_slot(clock.slot + 1)
        .expect("make the lookup-table slot recent");
    let payer = context.payer.pubkey();
    let (create, table) = create_lookup_table(payer, payer, clock.slot);
    process_legacy(context, create).await;
    for chunk in addresses.chunks(20) {
        process_legacy(
            context,
            extend_lookup_table(table, payer, Some(payer), chunk.to_vec()),
        )
        .await;
    }
    let extended = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("post-extension Clock");
    context
        .warp_to_slot(extended.slot + 1)
        .expect("activate the lookup addresses");
    table
}

async fn process_legacy(context: &mut ProgramTestContext, instruction: Instruction) {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("legacy blockhash");
    let transaction = solana_transaction::Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await
        .expect("lookup-table lifecycle must commit");
}

fn wire_extent(signatures: usize, message: &[u8]) -> usize {
    let extent = 1 + signatures * 64 + message.len();
    assert!(
        extent <= PACKET_DATA_BYTES,
        "the transaction serialises to {extent} bytes, past Solana's {PACKET_DATA_BYTES}-byte packet maximum"
    );
    extent
}

/// One submitted fill: whether it committed, its logs, its return data, its CU.
async fn submit(
    context: &mut ProgramTestContext,
    world: &World,
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Result<(bool, Vec<String>, Option<(Pubkey, Vec<u8>)>, u64), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &context.payer.pubkey(),
            std::slice::from_ref(&world.instruction),
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("v0 message"),
    );
    let transaction = VersionedTransaction::try_new(message, &[&context.payer, &world.taker])
        .expect("signed fill");
    let signature = transaction
        .signatures
        .first()
        .ok_or(BanksClientError::ClientError("unsigned transaction"))?
        .to_string();
    let wire_bytes = wire_extent(
        transaction.signatures.len(),
        &transaction.message.serialize(),
    );
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
    // The refusal is rendered from what the RUNTIME returned, never from what
    // the campaign expected.
    let failure = processed
        .result
        .clone()
        .err()
        .map(|error| format!("{error:?}"));
    let (logs, returned, units) = processed
        .metadata
        .map(|metadata| {
            (
                metadata.log_messages,
                metadata
                    .return_data
                    .map(|value| (value.program_id, value.data)),
                metadata.compute_units_consumed,
            )
        })
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
    Ok((accepted, logs, returned, units))
}

/// The refusal code the runtime actually returned, read out of its own logs.
fn refusal_code(logs: &[String]) -> Option<u32> {
    logs.iter()
        .find_map(|line| line.split("failed: custom program error: 0x").nth(1))
        .and_then(|code| u32::from_str_radix(code.trim(), 16).ok())
}

async fn drive(
    hostile: HostileV1,
    label: &str,
) -> (
    bool,
    Vec<String>,
    Option<(Pubkey, Vec<u8>)>,
    u64,
    Pubkey,
    ProgramTestContext,
) {
    let (test, world) = world(hostile);
    let mut context = test.start_with_context().await;
    let addresses = lookup_addresses(
        context.payer.pubkey(),
        world.taker.pubkey(),
        &world.instruction,
    );
    let table = create_live_lookup_table(&mut context, &addresses).await;
    let (accepted, logs, returned, units) = submit(&mut context, &world, table, &addresses, label)
        .await
        .expect("the fill must reach the runtime");
    (accepted, logs, returned, units, world.fund, context)
}

#[tokio::test]
async fn the_real_elf_executes_a_null_fill_and_advances_the_fund() {
    let (accepted, logs, returned, units, fund, context) = drive(
        HostileV1::None,
        "scoring dealer: null fill executes on the real ELF",
    )
    .await;
    assert!(accepted, "the null fill must commit: {logs:#?}");

    let (producer, receipt) = returned.expect("the route sets its receipt as return data");
    let receipt = DealerReceiptV1::decode(&receipt).expect("canonical Dealer receipt");
    assert_eq!(receipt.route, DealerRouteV1::Fill);
    assert_eq!(receipt.outcome_count, OUTCOME_COUNT);
    assert_eq!(receipt.fund_revision, FUND_REVISION + 1);
    assert_eq!((receipt.dealer_pays, receipt.dealer_receives), (0, 0));

    let committed = context
        .banks_client
        .get_account(fund)
        .await
        .expect("fund read")
        .expect("the fund survives its own fill");
    assert_eq!(committed.owner, producer, "the fund stays with its program");
    let committed = DealerFundV1::decode(&committed.data).expect("committed fund");
    assert_eq!(
        committed.revision,
        FUND_REVISION + 1,
        "the fill advances the fund exactly once"
    );
    assert_eq!(committed.cash, 5_000_000, "a null fill moves no cash");
    assert_eq!(
        hash(&committed.to_bytes().expect("bytes")).to_bytes(),
        receipt.fund_digest
    );
    // The route runs the LMSR kernel, five authenticators, two Claims record
    // decodes and three cash legs. The bound is a ceiling on the shape, not a
    // pin on the number: it exists so a route that doubles in cost is caught.
    assert!(
        units > 20_000,
        "the null fill cost only {units} compute units"
    );
    assert!(units < 200_000, "the null fill cost {units} compute units");
    println!("scoring dealer null fill: {units} compute units");
}

#[tokio::test]
async fn every_hostile_is_refused_by_its_own_name() {
    // One route, one code per concern. The pairs are the contract: a hostile
    // that starts returning a DIFFERENT code has moved which authenticator
    // catches it, and that is exactly as much a regression as a hostile that
    // stops being caught at all.
    let census = [
        (HostileV1::Wire, 0x4201_u32, "Request"),
        (HostileV1::FrameWidth, 0x4200, "Frame"),
        (HostileV1::TakerSubstituted, 0x4200, "Frame"),
        (HostileV1::FundOwner, 0x4207, "Fund"),
        (HostileV1::StaleRevision, 0x4208, "FundStale"),
        (HostileV1::WidthDisagreement, 0x420A, "Width"),
        (HostileV1::RuleUnsealed, 0x4204, "RuleSeal"),
        (HostileV1::MarketNotOpen, 0x4217, "Phase"),
        (HostileV1::ClaimsSubstituted, 0x4203, "Release"),
        (HostileV1::VaultSubstituted, 0x4213, "Custody"),
        (HostileV1::PositionSubstituted, 0x4209, "Position"),
        (HostileV1::TakerPositionOwner, 0x4214, "Claims"),
        (HostileV1::OffSchedule, 0x420E, "OffSchedule"),
        (HostileV1::NotNormalized, 0x420D, "NotNormalized"),
    ];
    for (hostile, expected, name) in census {
        let label = format!("scoring dealer: {hostile:?} refuses {name}");
        let (accepted, logs, _, units, _, _) = drive(hostile, &label).await;
        assert!(!accepted, "{hostile:?} must be refused: {logs:#?}");
        assert_eq!(
            refusal_code(&logs),
            Some(expected),
            "{hostile:?} must refuse {name} (0x{expected:04X}), not what it did: {logs:#?}"
        );
        println!("scoring dealer {hostile:?} -> {name} (0x{expected:04X}), {units} compute units");
    }
}
