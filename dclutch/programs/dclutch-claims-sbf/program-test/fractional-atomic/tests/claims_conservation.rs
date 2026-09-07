//! Real-ELF evidence for the Claims-owned conservation route, `DCLCNS01`:
//! split and merge as USER ACTS that move collateral.
//!
//! # What this campaign is, and what it replaced
//!
//! CLAIMS-18 stood here and proved, on the shipped ELF, that the route could
//! not execute: it read the aggregate and both Positions with decoders from
//! two disjoint account families -- the LBV2 records `founding_v5` writes
//! (`DCLLBM02`) and the economic slice's `DCLTEMK2` -- so no byte string
//! satisfied both readers, and the same frame refused two different ways
//! depending only on which family the aggregate's bytes belonged to.
//!
//! That finding is repealed, not forgotten. The route now reads ONE family,
//! reads the outstanding principal off the Custody `HoardPrincipal` vault
//! rather than a header scalar, and moves claims through the one LBV2
//! complete-set executor (`dclutch_claims::complete_set_v1`). Its frame is
//! twenty-one accounts, not twenty-nine: the linked basis record's own bytes
//! prove the Market's kind, width and payout scale, so the Product-graph walk
//! is gone. Its refusals are its own sub-band, `0x5300`-`0x530C`; nothing in
//! the tree raises `ClaimsSbfError::Economic` any more.
//!
//! # The world this campaign stands on, and what it establishes rather than plants
//!
//! Every account here comes from `founding_world` -- the same fixture
//! `claims_world.rs` uses -- so the Market this campaign splits and merges
//! on is one the FOUNDING ROUTE created on the real ELF, not one a fixture
//! wrote. Three things a founding leaves undone this campaign establishes, in
//! its own transactions, before any conservation act:
//!
//! 1. **The Claims-role Custody replay.** A founding advances the TRADING-role
//!    cursor; a split or a merge is a Claims-role Custody effect and needs its
//!    own. This campaign creates it by driving the Claims program's own
//!    `DCLCCR01` route, which is what the runbook's
//!    `devnet-claims-custody-replay-v1` verb drives -- the precondition is
//!    executed, not planted.
//! 2. **The Market opens.** A founding consumes a Market in `Phase::Founding`
//!    and Claims cannot write Core's account. The phase advance is Core's own
//!    stage and no part of this campaign's subject, so it is applied to the
//!    Core account directly, through the codec that owns `CoreState`, leaving
//!    every other field -- including the identity the account's own address
//!    derives from -- exactly as the founding found it.
//! 3. **A stranger's donation to the vault.** L4 is an INEQUALITY:
//!    `vault_atoms >= max_k supply[k] * basis_scale`. A real Token-2022
//!    transfer from a stranger into the vault, of an amount that is not a
//!    multiple of the basis scale, is what makes that an inequality under test
//!    rather than an equality that happens to hold.
//!
//! The actor is the FOUNDER, and that is not a convenience: a Position is
//! created by admission, so the only holder who can merge on a freshly founded
//! Market is the one the founding admitted. The founder also PAYS, signing as
//! a writable fee payer -- the single-wallet shape the route's privilege pass
//! deliberately admits.
//!
//! # The census
//!
//! The round trip is measured by the journey's eight laws, restated here over
//! the accounts a program-test can read. `the_census_reads_red_when_a_law_is_broken`
//! is their positive control: a census that cannot report VIOLATED proves
//! nothing by reporting HOLDS.

use std::collections::BTreeMap;

use dclutch_claims::complete_set_v1::{failure_selector_v1, held_complete_sets_v1};
use dclutch_claims::conservation::{
    ClaimsConservationDirectionV1, ClaimsConservationRequestV1, frame_v1,
};
use dclutch_claims::custody_replay_v1::ClaimsCustodyReplayRequestV1;
use dclutch_claims::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_claims_sbf::claims_conservation_v1::ClaimsConservationSbfErrorV1;
use dclutch_claims_sbf::custody_replay_v1 as replay_route;
use dclutch_custody::CUSTODY_REPLAY_BYTES_V1;
use dclutch_custody::token_svm::instruction::transfer_checked;
use dclutch_fractional_atomic_program_test::campaign_support::{
    add_account, mint_supply, programdata_address, token_account_bytes_for, token_amount,
    token_program_id,
};
use dclutch_fractional_atomic_program_test::founding_world::{
    CLAIMS_PROGRAM_ID, CLAIM_COUNT, COLLATERAL_MINT, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID,
    FoundingShapeV1, FoundingWorld, HostileV1, Outcome, QUANTITY, REGISTRY_PROGRAM_ID,
    founder_keypair, founding_instruction, submit, submit_with, world_with_extra_collateral,
};
use dclutch_market::{CoreState, Phase};
use dclutch_operator::claims_conservation_v1::{
    ClaimsConservationActV1, ClaimsConservationObservedV1, ClaimsConservationPlanV1,
    ClaimsConservationProgramsV1, plan_claims_conservation_v1,
};
use solana_account::AccountSharedData;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{signature::Keypair, signer::Signer};
use solana_sdk_ids::{system_program, sysvar};

// ---------------------------------------------------------------------------
// The campaign's own figures
// ---------------------------------------------------------------------------

/// Complete sets the split creates and the merge destroys.
///
/// Deliberately NOT the founding's `QUANTITY`: an act of the same size as the
/// issuance would let a poststate that mistook one for the other read correct.
const SPLIT_SETS: u64 = 2;

/// A stranger's donation to the vault, in atoms.
///
/// Not a multiple of the basis scale (3), so no reading of it as complete sets
/// is available and the excess it leaves is unambiguously excess.
const DONATION_ATOMS: u64 = 5;

/// The collateral mint's decimals, as `founding_world` mints it.
const COLLATERAL_DECIMALS: u8 = 6;

/// The founder's own collateral account, funded to exactly the split's cost.
const FOUNDER_COLLATERAL: Pubkey = Pubkey::new_from_array([0xc1; 32]);

/// A stranger's collateral account, funded only to donate.
const STRANGER_COLLATERAL: Pubkey = Pubkey::new_from_array([0xc2; 32]);

fn stranger_keypair() -> Keypair {
    Keypair::new_from_array([0x21; 32])
}

/// A refunding Market's payout scale, which is also its basis scale.
const fn refunding_scale() -> u64 {
    (CLAIM_COUNT - 1) as u64
}

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

/// Build the founding world with the extra collateral this campaign spends.
///
/// The atoms beyond the founding's own are minted by
/// `world_with_extra_collateral`; this seats them in two accounts, so the
/// collateral total the census closes over is the Mint's whole supply.
fn conservation_world(
    shape: FoundingShapeV1,
    founder_atoms: u64,
    stranger_atoms: u64,
) -> (ProgramTest, FoundingWorld) {
    let (mut test, founding) = world_with_extra_collateral(
        shape,
        HostileV1::None,
        founder_atoms
            .checked_add(stranger_atoms)
            .expect("extra collateral"),
    );
    add_account(
        &mut test,
        FOUNDER_COLLATERAL,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, founder_keypair().pubkey(), founder_atoms),
    );
    add_account(
        &mut test,
        STRANGER_COLLATERAL,
        token_program_id(),
        token_account_bytes_for(COLLATERAL_MINT, stranger_keypair().pubkey(), stranger_atoms),
    );
    (test, founding)
}

/// The programs every plan on this world addresses.
fn programs() -> ClaimsConservationProgramsV1 {
    ClaimsConservationProgramsV1 {
        claims: CLAIMS_PROGRAM_ID,
        claims_programdata: programdata_address(CLAIMS_PROGRAM_ID),
        custody: CUSTODY_PROGRAM_ID,
        core: CORE_PROGRAM_ID,
        registry: REGISTRY_PROGRAM_ID,
    }
}

// ---------------------------------------------------------------------------
// Reading the chain
// ---------------------------------------------------------------------------

async fn account_bytes(context: &mut ProgramTestContext, key: Pubkey) -> Vec<u8> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .unwrap_or_else(|| panic!("account {key} exists"))
        .data
}

/// The bytes of an account that may not exist -- a categorical Market's failure
/// escrow is derived, named in every frame, and never created.
async fn optional_account_bytes(
    context: &mut ProgramTestContext,
    key: Pubkey,
) -> Option<Vec<u8>> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .map(|account| account.data)
}

async fn account_lamports(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .map_or(0, |account| account.lamports)
}

async fn account_exists(context: &mut ProgramTestContext, key: Pubkey) -> bool {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .is_some()
}

/// Every supply coordinate of the LBV2 aggregate.
fn aggregate_supply(bytes: &[u8]) -> Vec<u64> {
    let view = LiabilityBasisMarketViewV2::decode(bytes).expect("LBV2 aggregate");
    (0..view.claim_count)
        .map(|outcome| view.supply(bytes, outcome).expect("supply"))
        .collect()
}

/// Every balance coordinate of one LBV2 Position.
fn position_balances(bytes: &[u8]) -> Vec<u64> {
    let view = LiabilityBasisPositionViewV2::decode(bytes).expect("LBV2 Position");
    (0..view.claim_count)
        .map(|outcome| view.balance(bytes, outcome).expect("balance"))
        .collect()
}

/// The coordinate-wise sum of the Positions this campaign names.
fn position_totals(positions: &[Vec<u64>], claim_count: u32) -> Vec<u64> {
    (0..usize::try_from(claim_count).expect("width"))
        .map(|outcome| {
            positions
                .iter()
                .map(|balances| balances.get(outcome).copied().unwrap_or(0))
                .sum()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The eight laws, over what a program-test can read
// ---------------------------------------------------------------------------

/// One law's reading at one boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
struct VerdictV1 {
    law: &'static str,
    status: &'static str,
    detail: String,
}

impl VerdictV1 {
    fn holds(law: &'static str, detail: String) -> Self {
        Self {
            law,
            status: "holds",
            detail,
        }
    }

    fn violated(law: &'static str, detail: String) -> Self {
        Self {
            law,
            status: "violated",
            detail,
        }
    }

    fn inapplicable(law: &'static str, detail: &str) -> Self {
        Self {
            law,
            status: "inapplicable",
            detail: detail.to_owned(),
        }
    }
}

/// One observation of the collateral world at one boundary.
#[derive(Clone, Debug)]
struct ObservationV1 {
    stage: &'static str,
    mint_supply: u64,
    /// Every collateral token account this campaign names, by label.
    token_atoms: BTreeMap<&'static str, u64>,
    /// Atoms in every vault, by the compartment class its PDA seeds name.
    class_atoms: BTreeMap<&'static str, u64>,
    /// Lamports and existence of every account whose closure L6 would notice.
    accounts: BTreeMap<&'static str, (bool, u64)>,
    aggregate_supply: Vec<u64>,
    position_totals: Vec<u64>,
    claim_unit_atoms: u64,
    payer: Pubkey,
    payer_lamports: u64,
    /// What the stage says it moved, per class. L2 reads `HoardPrincipal`.
    declared_class_deltas: BTreeMap<&'static str, i128>,
    /// What the stage says the tracked total moved.
    declared_collateral_delta: i128,
    /// The fee the bank quoted, when L7 is stated at this boundary at all.
    declared_fee_lamports: Option<u64>,
    /// Why L7 does not apply here, when it does not.
    lamports_inapplicable: Option<&'static str>,
}

impl ObservationV1 {
    fn tracked_collateral(&self) -> u64 {
        self.token_atoms.values().sum()
    }

    fn hoard_atoms(&self) -> u64 {
        self.class_atoms
            .get("HoardPrincipal")
            .copied()
            .unwrap_or_default()
    }
}

/// The eight laws, evaluated against the previous observation.
#[derive(Default)]
struct CensusV1 {
    observations: Vec<ObservationV1>,
}

impl CensusV1 {
    fn evaluate(&self, now: &ObservationV1) -> Vec<VerdictV1> {
        let previous = self.observations.last();
        let mut verdicts = Vec::new();

        let tracked = now.tracked_collateral();
        verdicts.push(if tracked == now.mint_supply {
            VerdictV1::holds(
                "L1",
                format!(
                    "tracked {tracked} atoms across {} accounts == Mint supply {}",
                    now.token_atoms.len(),
                    now.mint_supply
                ),
            )
        } else {
            VerdictV1::violated(
                "L1",
                format!(
                    "tracked {tracked} atoms across {} accounts != Mint supply {}",
                    now.token_atoms.len(),
                    now.mint_supply
                ),
            )
        });

        verdicts.push(match previous {
            None => VerdictV1::inapplicable("L2", "the first census has no predecessor"),
            Some(before) => {
                let observed = i128::from(now.hoard_atoms()) - i128::from(before.hoard_atoms());
                let declared = now
                    .declared_class_deltas
                    .get("HoardPrincipal")
                    .copied()
                    .unwrap_or_default();
                if observed == declared {
                    VerdictV1::holds(
                        "L2",
                        format!(
                            "the Hoard moved {observed} atoms since `{}`, exactly as declared; it holds {}",
                            before.stage,
                            now.hoard_atoms()
                        ),
                    )
                } else {
                    VerdictV1::violated(
                        "L2",
                        format!(
                            "the Hoard moved {observed} atoms since `{}` and the stage declared {declared}",
                            before.stage
                        ),
                    )
                }
            }
        });

        verdicts.push(if now.position_totals == now.aggregate_supply {
            VerdictV1::holds(
                "L3",
                format!(
                    "the Positions sum to the aggregate supply vector {:?}",
                    now.aggregate_supply
                ),
            )
        } else {
            VerdictV1::violated(
                "L3",
                format!(
                    "the Positions sum to {:?} but the aggregate owes {:?}",
                    now.position_totals, now.aggregate_supply
                ),
            )
        });

        verdicts.push({
            let worst = now.aggregate_supply.iter().max().copied().unwrap_or(0);
            match worst.checked_mul(now.claim_unit_atoms) {
                None => VerdictV1::violated(
                    "L4",
                    format!(
                        "worst outcome {worst} claims at {} atoms each overflows u64",
                        now.claim_unit_atoms
                    ),
                ),
                Some(required) if now.hoard_atoms() >= required => VerdictV1::holds(
                    "L4",
                    format!(
                        "Hoard {} >= worst outcome {worst} x unit {} = {required}",
                        now.hoard_atoms(),
                        now.claim_unit_atoms
                    ),
                ),
                Some(required) => VerdictV1::violated(
                    "L4",
                    format!(
                        "Hoard {} < worst outcome {worst} x unit {} = {required}; the Market is under-collateralised",
                        now.hoard_atoms(),
                        now.claim_unit_atoms
                    ),
                ),
            }
        });

        verdicts.push(match previous {
            None => VerdictV1::inapplicable("L5", "the first census has no predecessor"),
            Some(before) => {
                let observed =
                    i128::from(tracked) - i128::from(before.tracked_collateral());
                if observed == now.declared_collateral_delta {
                    VerdictV1::holds(
                        "L5",
                        format!(
                            "tracked collateral moved {observed} atoms since `{}`, exactly as declared",
                            before.stage
                        ),
                    )
                } else {
                    VerdictV1::violated(
                        "L5",
                        format!(
                            "tracked collateral moved {observed} atoms since `{}`; the stage declared {}",
                            before.stage, now.declared_collateral_delta
                        ),
                    )
                }
            }
        });

        verdicts.push(match previous {
            None => VerdictV1::inapplicable("L6", "the first census has no predecessor"),
            Some(before) => {
                let vanished: Vec<String> = before
                    .accounts
                    .iter()
                    .filter_map(|(label, (existed, lamports))| {
                        let (exists, _) = now.accounts.get(label)?;
                        (*existed && !*exists && *lamports > 0)
                            .then(|| format!("{label} ({lamports} lamports)"))
                    })
                    .collect();
                if vanished.is_empty() {
                    VerdictV1::holds("L6", "no watched account closed at this boundary".into())
                } else {
                    VerdictV1::violated(
                        "L6",
                        format!("watched accounts closed unaccounted: {}", vanished.join(", ")),
                    )
                }
            }
        });

        verdicts.push(match (previous, now.lamports_inapplicable, now.declared_fee_lamports) {
            (None, _, _) => VerdictV1::inapplicable("L7", "the first census has no predecessor"),
            (_, Some(reason), _) => VerdictV1::inapplicable("L7", reason),
            (_, None, None) => VerdictV1::inapplicable(
                "L7",
                "the bank quoted no fee for this message, so there is no declared side to hold \
                 the payer's lamports to",
            ),
            (Some(before), None, Some(fee)) => {
                if before.payer != now.payer {
                    VerdictV1::inapplicable(
                        "L7",
                        "the payer changed at this boundary, and one wallet's delta cannot be \
                         read against another's",
                    )
                } else {
                    let observed = i128::from(before.payer_lamports) - i128::from(now.payer_lamports);
                    if observed == i128::from(fee) {
                        VerdictV1::holds(
                            "L7",
                            format!(
                                "the payer's lamports fell by exactly the {fee}-lamport fee since `{}`",
                                before.stage
                            ),
                        )
                    } else {
                        VerdictV1::violated(
                            "L7",
                            format!(
                                "the payer's lamports fell {observed} since `{}` and the fee was {fee}",
                                before.stage
                            ),
                        )
                    }
                }
            }
        });

        verdicts.push(match previous {
            None => VerdictV1::inapplicable("L8", "the first census has no predecessor"),
            Some(before) => {
                let mut classes: Vec<&str> = before
                    .class_atoms
                    .keys()
                    .chain(now.class_atoms.keys())
                    .copied()
                    .collect();
                classes.sort_unstable();
                classes.dedup();
                let mut breaches = Vec::new();
                let mut held = Vec::new();
                for class in classes {
                    let was = i128::from(before.class_atoms.get(class).copied().unwrap_or(0));
                    let is = i128::from(now.class_atoms.get(class).copied().unwrap_or(0));
                    let observed = is - was;
                    let declared = now.declared_class_deltas.get(class).copied().unwrap_or(0);
                    if observed == declared {
                        held.push(format!("{class} {observed:+}"));
                    } else {
                        breaches.push(format!(
                            "{class} moved {observed:+} atoms and the stage declared {declared:+}"
                        ));
                    }
                }
                if breaches.is_empty() {
                    VerdictV1::holds(
                        "L8",
                        format!(
                            "every compartment moved exactly as declared since `{}`: {}",
                            before.stage,
                            held.join(", ")
                        ),
                    )
                } else {
                    VerdictV1::violated("L8", breaches.join("; "))
                }
            }
        });

        verdicts
    }

    /// Evaluate, require every law to HOLD or be honestly INAPPLICABLE, and
    /// keep the observation as the next boundary's predecessor.
    fn admit(&mut self, now: ObservationV1) {
        let verdicts = self.evaluate(&now);
        for verdict in &verdicts {
            assert_ne!(
                verdict.status, "violated",
                "{} VIOLATED at `{}`: {}",
                verdict.law, now.stage, verdict.detail,
            );
            println!("  {} {} at `{}`  {}", verdict.law, verdict.status, now.stage, verdict.detail);
        }
        self.observations.push(now);
    }
}

/// Observe the whole collateral world at one boundary.
#[allow(clippy::too_many_arguments)]
async fn observe(
    context: &mut ProgramTestContext,
    world: &FoundingWorld,
    stage: &'static str,
    payer: Pubkey,
    declared_hoard_delta: i128,
    declared_collateral_delta: i128,
    declared_fee_lamports: Option<u64>,
    lamports_inapplicable: Option<&'static str>,
) -> ObservationV1 {
    let mint = account_bytes(context, COLLATERAL_MINT).await;
    let hoard = token_amount(&account_bytes(context, world.hoard).await);
    let founder = token_amount(&account_bytes(context, FOUNDER_COLLATERAL).await);
    let stranger = token_amount(&account_bytes(context, STRANGER_COLLATERAL).await);
    let aggregate = account_bytes(context, world.aggregate).await;
    let holder = position_balances(&account_bytes(context, world.position).await);
    let escrow = position_balances(&account_bytes(context, world.escrow_position).await);
    let supply = aggregate_supply(&aggregate);
    let claim_count = LiabilityBasisMarketViewV2::decode(&aggregate)
        .expect("LBV2 aggregate")
        .claim_count;

    let mut token_atoms = BTreeMap::new();
    token_atoms.insert("hoard", hoard);
    token_atoms.insert("founder", founder);
    token_atoms.insert("stranger", stranger);

    let mut class_atoms = BTreeMap::new();
    class_atoms.insert("HoardPrincipal", hoard);

    let mut accounts = BTreeMap::new();
    for (label, key) in [
        ("aggregate", world.aggregate),
        ("position", world.position),
        ("escrow", world.escrow_position),
        ("hoard", world.hoard),
        ("founder-collateral", FOUNDER_COLLATERAL),
        ("stranger-collateral", STRANGER_COLLATERAL),
    ] {
        accounts.insert(
            label,
            (
                account_exists(context, key).await,
                account_lamports(context, key).await,
            ),
        );
    }

    let mut declared_class_deltas = BTreeMap::new();
    declared_class_deltas.insert("HoardPrincipal", declared_hoard_delta);

    ObservationV1 {
        stage,
        mint_supply: mint_supply(&mint),
        token_atoms,
        class_atoms,
        accounts,
        aggregate_supply: supply,
        position_totals: position_totals(&[holder, escrow], claim_count),
        claim_unit_atoms: refunding_scale(),
        payer,
        payer_lamports: account_lamports(context, payer).await,
        declared_class_deltas,
        declared_collateral_delta,
        declared_fee_lamports,
        lamports_inapplicable,
    }
}

// ---------------------------------------------------------------------------
// The three preconditions, each in its own transaction
// ---------------------------------------------------------------------------

/// Drive the founding on the real ELF and require it accepted.
async fn found(context: &mut ProgramTestContext, world: &FoundingWorld, label: &str) {
    let outcome = submit(context, label, founding_instruction(world)).await;
    assert!(
        outcome.accepted,
        "the conservation campaign stands on a real founding; it refused {:?}: {:?}",
        outcome.refusal, outcome.logs,
    );
}

/// The Claims-role Custody replay this Market's split and merge serialize on.
///
/// The frame is the route's own, by its own coordinate constants, and the
/// Custody request is built by the route's own `expected_request_v1` -- so the
/// caller-authority PDA this campaign derives and the one the program derives
/// have exactly one author.
async fn claims_replay_instruction(
    context: &mut ProgramTestContext,
    world: &FoundingWorld,
    payer: Pubkey,
) -> (Instruction, Pubkey) {
    // The aggregate is read off the CHAIN, not off the fixture: the founding
    // route writes `custody_context` from the projection it authenticated, and
    // the fixture body the narrow compiler produced carries a different one.
    // Every coordinate below hangs off that field.
    let aggregate_bytes = account_bytes(context, world.aggregate).await;
    let view = LiabilityBasisMarketViewV2::decode(&aggregate_bytes)
        .expect("the founded aggregate decodes as LBV2");
    let request = replay_route::expected_request_v1(
        view,
        CLAIMS_PROGRAM_ID.to_bytes(),
        payer.to_bytes(),
        world.rent_credit.to_bytes(),
        Rent::default().minimum_balance(CUSTODY_REPLAY_BYTES_V1),
    )
    .expect("the replay route's sole Custody request");
    let request_bytes = request.to_bytes().expect("Custody request bytes");
    let request_digest = solana_program::hash::hash(&request_bytes).to_bytes();
    let caller_authority = Pubkey::find_program_address(
        &dclutch_registry::release_set::CallerAuthoritySeedsV1::from_bytes(
            request.release_set,
            request.market,
            dclutch_registry::release_set::ExecutionRoleV1::Claims,
            request.context,
            request_digest,
        )
        .expect("claims-role caller seeds")
        .as_slices(),
        &CLAIMS_PROGRAM_ID,
    )
    .0;
    let replay = Pubkey::find_program_address(
        &dclutch_custody::CustodyReplaySeedsV1::from_request(request).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let mut accounts =
        vec![AccountMeta::new_readonly(Pubkey::default(), false); replay_route::CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1];
    let put = |accounts: &mut Vec<AccountMeta>, index: usize, meta: AccountMeta| {
        *accounts.get_mut(index).expect("frame coordinate") = meta;
    };
    put(
        &mut accounts,
        replay_route::CUSTODY_CALLER_AUTHORITY,
        AccountMeta::new_readonly(caller_authority, false),
    );
    put(
        &mut accounts,
        replay_route::CORE_MARKET,
        AccountMeta::new_readonly(world.shared.core_market, false),
    );
    put(
        &mut accounts,
        replay_route::ACTIVATION_CACHE,
        AccountMeta::new_readonly(world.activation_cache, false),
    );
    put(
        &mut accounts,
        replay_route::REGISTRY_PROGRAM,
        AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
    );
    put(
        &mut accounts,
        replay_route::CLAIMS_PROGRAM,
        AccountMeta::new_readonly(CLAIMS_PROGRAM_ID, false),
    );
    put(
        &mut accounts,
        replay_route::CLAIMS_PROGRAMDATA,
        AccountMeta::new_readonly(programdata_address(CLAIMS_PROGRAM_ID), false),
    );
    put(
        &mut accounts,
        replay_route::REALM,
        AccountMeta::new_readonly(world.realm_raw, false),
    );
    put(
        &mut accounts,
        replay_route::REALM_STAGING,
        AccountMeta::new_readonly(world.realm_staging, false),
    );
    put(
        &mut accounts,
        replay_route::CUSTODY_REPLAY,
        AccountMeta::new(replay, false),
    );
    put(&mut accounts, replay_route::PAYER, AccountMeta::new(payer, true));
    put(
        &mut accounts,
        replay_route::SYSTEM_PROGRAM,
        AccountMeta::new_readonly(system_program::ID, false),
    );
    put(
        &mut accounts,
        replay_route::RENT_SYSVAR,
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    );
    put(
        &mut accounts,
        replay_route::RENT_REFUND,
        AccountMeta::new(world.rent_credit, false),
    );
    put(
        &mut accounts,
        replay_route::CUSTODY_PROGRAM,
        AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
    );
    put(
        &mut accounts,
        replay_route::AGGREGATE,
        AccountMeta::new_readonly(world.aggregate, false),
    );
    (
        Instruction {
            program_id: CLAIMS_PROGRAM_ID,
            accounts,
            data: ClaimsCustodyReplayRequestV1::new(world.shared.core_market.to_bytes())
                .expect("canonical DCLCCR01 request")
                .to_bytes()
                .to_vec(),
        },
        replay,
    )
}

/// Create the Claims-role replay and require it accepted.
async fn open_claims_replay(
    context: &mut ProgramTestContext,
    world: &FoundingWorld,
    label: &str,
) -> Pubkey {
    let payer = context.payer.pubkey();
    let (instruction, replay) = claims_replay_instruction(context, world, payer).await;
    let outcome = submit(context, label, instruction).await;
    assert!(
        outcome.accepted,
        "the Claims-role Custody replay is this route's precondition and its own route \
         creates it; it refused {:?}: {:?}",
        outcome.refusal, outcome.logs,
    );
    replay
}

/// Advance the Core Market from `Founding` to `Open`.
///
/// Core's own stage, and no part of this campaign's subject. Every other field
/// -- including `identity`, which the Market account's address derives from --
/// is left exactly as the founding found it.
async fn open_the_market(context: &mut ProgramTestContext, world: &FoundingWorld) {
    let key = world.shared.core_market;
    let mut account = context
        .banks_client
        .get_account(key)
        .await
        .expect("bank read")
        .expect("the Core Market exists");
    let mut core = CoreState::decode(&account.data).expect("Core state");
    core.phase = Phase::Open;
    account.data = core.encode().expect("an Open Core state").to_vec();
    context.set_account(&key, &AccountSharedData::from(account));
}

/// A stranger's real Token-2022 transfer into the Market's vault.
async fn donate_to_the_vault(
    context: &mut ProgramTestContext,
    world: &FoundingWorld,
    atoms: u64,
    label: &str,
) {
    let spec = transfer_checked(
        token_program_id().to_bytes(),
        STRANGER_COLLATERAL.to_bytes(),
        COLLATERAL_MINT.to_bytes(),
        world.hoard.to_bytes(),
        stranger_keypair().pubkey().to_bytes(),
        atoms,
        COLLATERAL_DECIMALS,
    )
    .expect("a checked transfer of the Realm's own collateral");
    let instruction = Instruction {
        program_id: Pubkey::new_from_array(*spec.program_id()),
        accounts: spec
            .accounts()
            .iter()
            .map(|account| AccountMeta {
                pubkey: Pubkey::new_from_array(*account.address()),
                is_signer: account.is_signer(),
                is_writable: account.is_writable(),
            })
            .collect(),
        data: spec.data().to_vec(),
    };
    let payer = context.payer.insecure_clone();
    let outcome = submit_with(
        context,
        label,
        &[instruction],
        &payer,
        &[&stranger_keypair()],
    )
    .await;
    assert!(
        outcome.accepted,
        "a token account accepts a transfer from anybody, which is why L4 is an inequality; \
         it refused {:?}: {:?}",
        outcome.refusal, outcome.logs,
    );
}

// ---------------------------------------------------------------------------
// Planning and submitting one conservation act
// ---------------------------------------------------------------------------

/// Plan one act from the chain's own bytes, through the operator.
///
/// The operator is the one author of the twenty-one-account frame and of the
/// `ApproveChecked` a split owes; this campaign never writes a second copy of
/// either, so a frame that drifts from the route breaks the operator's own
/// tests first and this campaign second.
async fn plan(
    context: &mut ProgramTestContext,
    world: &FoundingWorld,
    direction: ClaimsConservationDirectionV1,
    quantity: u64,
) -> ClaimsConservationPlanV1 {
    let core_state = account_bytes(context, world.shared.core_market).await;
    let aggregate = account_bytes(context, world.aggregate).await;
    let position = account_bytes(context, world.position).await;
    let escrow = optional_account_bytes(context, world.escrow_position).await;
    let basis = account_bytes(context, world.shared.linked_basis.raw).await;
    let replay_key = Pubkey::find_program_address(
        &dclutch_custody::CustodyReplaySeedsV1::new(
            world.shared.core_market.to_bytes(),
            world.release_set,
            dclutch_custody::CallerRoleV1::Claims,
            LiabilityBasisMarketViewV2::decode(&aggregate)
                .expect("LBV2 aggregate")
                .custody_context,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .0;
    let replay = account_bytes(context, replay_key).await;
    let hoard = account_bytes(context, world.hoard).await;
    let external = account_bytes(context, FOUNDER_COLLATERAL).await;
    let mint = account_bytes(context, COLLATERAL_MINT).await;
    plan_claims_conservation_v1(
        programs(),
        ClaimsConservationObservedV1 {
            market: world.shared.core_market,
            core_state: &core_state,
            aggregate: &aggregate,
            position: &position,
            escrow_position: escrow.as_deref(),
            basis_record: (world.shared.linked_basis.raw, &basis),
            custody_replay: &replay,
            hoard_vault: &hoard,
            external_collateral: (FOUNDER_COLLATERAL, &external),
            collateral_mint: (COLLATERAL_MINT, &mint),
            token_program: token_program_id(),
            realm_raw: world.realm_raw,
            realm_staging: world.realm_staging,
        },
        ClaimsConservationActV1 {
            direction,
            owner: founder_keypair().pubkey(),
            quantity,
        },
    )
    .expect("the operator plans this act from the chain's own bytes")
}

/// Submit one planned act, paid and signed by the owner alone.
///
/// The owner is a WRITABLE SIGNER here on purpose: a wallet that authorizes its
/// own act pays the transaction's fee, and the route's privilege pass admits
/// exactly that. An earlier draft of the route pinned the owner `!is_writable`,
/// which no single-wallet submitter can satisfy.
async fn submit_act(
    context: &mut ProgramTestContext,
    plan: &ClaimsConservationPlanV1,
    label: &str,
) -> Outcome {
    let mut instructions = Vec::new();
    if let Some(approve) = plan.approve.clone() {
        instructions.push(approve);
    }
    instructions.push(plan.instruction.clone());
    let owner = founder_keypair();
    submit_with(context, label, &instructions, &owner, &[]).await
}

/// Submit one instruction the operator would never build, paid by the owner.
async fn submit_hostile(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    label: &str,
) -> Outcome {
    let owner = founder_keypair();
    submit_with(context, label, instructions, &owner, &[]).await
}

/// Stand the world up to the point where a conservation act is admissible.
async fn founded_and_open(
    shape: FoundingShapeV1,
    founder_atoms: u64,
    stranger_atoms: u64,
    label: &str,
) -> (FoundingWorld, ProgramTestContext) {
    let (test, world) = conservation_world(shape, founder_atoms, stranger_atoms);
    let mut context = test.start_with_context().await;
    found(&mut context, &world, &format!("{label}: founding")).await;
    open_claims_replay(&mut context, &world, &format!("{label}: claims-role replay")).await;
    open_the_market(&mut context, &world).await;
    (world, context)
}

// ---------------------------------------------------------------------------
// The round trip
// ---------------------------------------------------------------------------

/// A split moves collateral into the vault and mints a complete set; its merge
/// undoes both. L1-L8 hold at every boundary.
///
/// This is the whole claim of the family: split and merge are user acts that
/// MOVE COLLATERAL through the Custody transfer, and the round trip is the
/// identity on every account it touches but for the three revisions.
#[tokio::test]
async fn a_split_and_its_merge_are_a_round_trip_over_the_eight_laws() {
    let split_atoms = SPLIT_SETS * refunding_scale();
    let (world, mut context) = founded_and_open(
        FoundingShapeV1::Refunding,
        split_atoms,
        DONATION_ATOMS,
        "conservation round trip",
    )
    .await;
    let founder = founder_keypair().pubkey();
    let mut census = CensusV1::default();

    census.admit(
        observe(
            &mut context,
            &world,
            "founded",
            founder,
            0,
            0,
            None,
            Some(
                "the founding and the replay were paid by the bank's payer and created three \
                 accounts from prepaid rent; that lamport story is the founding campaign's",
            ),
        )
        .await,
    );

    donate_to_the_vault(
        &mut context,
        &world,
        DONATION_ATOMS,
        "conservation round trip: a stranger donates to the vault",
    )
    .await;
    census.admit(
        observe(
            &mut context,
            &world,
            "donated",
            founder,
            i128::from(DONATION_ATOMS),
            0,
            None,
            Some("the donation was paid by the bank's payer, not the owner"),
        )
        .await,
    );

    let before = aggregate_supply(&account_bytes(&mut context, world.aggregate).await);
    let split = plan(
        &mut context,
        &world,
        ClaimsConservationDirectionV1::Split,
        SPLIT_SETS,
    )
    .await;
    assert_eq!(
        split.collateral_atoms, split_atoms,
        "a split's collateral is exactly `quantity * basis_scale` atoms",
    );
    assert!(
        split.refunds_on_failure,
        "the record, not the caller, says this Market refunds",
    );
    assert!(
        split.approve.is_some(),
        "a split debits the actor's own account, so it owes the delegated Custody wire and \
         the ApproveChecked that makes it admissible",
    );
    let outcome = submit_act(&mut context, &split, "conservation: the split").await;
    assert!(
        outcome.accepted,
        "the split refused {:?}: {:?}",
        outcome.refusal, outcome.logs,
    );
    println!("conservation split: accepted, {} CU consumed", outcome.units);
    census.admit(
        observe(
            &mut context,
            &world,
            "split",
            founder,
            i128::from(split_atoms),
            0,
            outcome.fee_lamports,
            None,
        )
        .await,
    );

    let after_split =
        aggregate_supply(&account_bytes(&mut context, world.aggregate).await);
    for (coordinate, (was, is)) in before.iter().zip(after_split.iter()).enumerate() {
        assert_eq!(
            *is,
            was + SPLIT_SETS,
            "the split credited coordinate {coordinate} by exactly the sets it minted",
        );
    }
    let failure = failure_selector_v1(CLAIM_COUNT).expect("failure selector");
    let holder = position_balances(&account_bytes(&mut context, world.position).await);
    let escrow =
        position_balances(&account_bytes(&mut context, world.escrow_position).await);
    assert_eq!(
        holder
            .get(usize::try_from(failure).expect("selector"))
            .copied(),
        Some(0),
        "on a refunding Market the holder never receives the failure coordinate",
    );
    assert_eq!(
        escrow
            .get(usize::try_from(failure).expect("selector"))
            .copied(),
        Some(QUANTITY + SPLIT_SETS),
        "the failure coordinate of the split's set is seated in the Market's own escrow",
    );

    let merge = plan(
        &mut context,
        &world,
        ClaimsConservationDirectionV1::Merge,
        SPLIT_SETS,
    )
    .await;
    assert_eq!(
        merge.collateral_atoms, split_atoms,
        "the merge returns exactly the atoms the split took",
    );
    assert!(
        merge.approve.is_none(),
        "a merge debits the vault, which Custody's plain V1 Transfer already authorizes",
    );
    let outcome = submit_act(&mut context, &merge, "conservation: the merge").await;
    assert!(
        outcome.accepted,
        "the merge refused {:?}: {:?}",
        outcome.refusal, outcome.logs,
    );
    println!("conservation merge: accepted, {} CU consumed", outcome.units);
    census.admit(
        observe(
            &mut context,
            &world,
            "merged",
            founder,
            -i128::from(split_atoms),
            0,
            outcome.fee_lamports,
            None,
        )
        .await,
    );

    let after_merge =
        aggregate_supply(&account_bytes(&mut context, world.aggregate).await);
    assert_eq!(
        after_merge, before,
        "the round trip is the identity on the aggregate's supply vector",
    );
    assert_eq!(
        token_amount(&account_bytes(&mut context, FOUNDER_COLLATERAL).await),
        split_atoms,
        "and on the actor's own collateral",
    );
    assert_eq!(
        token_amount(&account_bytes(&mut context, world.hoard).await),
        QUANTITY * refunding_scale() + DONATION_ATOMS,
        "and on the vault, which keeps the stranger's donation it never owed anybody",
    );
    assert_eq!(
        held_complete_sets_v1(
            &account_bytes(&mut context, world.position).await,
            true
        ),
        Ok(QUANTITY),
        "the holder is back to the sets the founding admitted",
    );
}

// ---------------------------------------------------------------------------
// The hostiles
// ---------------------------------------------------------------------------

/// A split whose actor does not hold the collateral refuses `Balances`, before
/// anything moves.
///
/// This is the conjunct that stops claims from existing against a vault that
/// has not received their backing. The request states a prestate the account
/// does not hold; every other coordinate is the operator's own.
#[tokio::test]
async fn a_split_without_the_collateral_refuses_balances() {
    let split_atoms = SPLIT_SETS * refunding_scale();
    let (world, mut context) = founded_and_open(
        FoundingShapeV1::Refunding,
        split_atoms,
        0,
        "conservation hostile: a split without collateral",
    )
    .await;
    let plan = plan(
        &mut context,
        &world,
        ClaimsConservationDirectionV1::Split,
        SPLIT_SETS,
    )
    .await;
    // The plan was true when it was made; the actor spends one atom elsewhere
    // before submitting it, so the stated prestate is one atom too large.
    let mut account = context
        .banks_client
        .get_account(FOUNDER_COLLATERAL)
        .await
        .expect("bank read")
        .expect("the actor's collateral account");
    account.data = token_account_bytes_for(
        COLLATERAL_MINT,
        founder_keypair().pubkey(),
        split_atoms - 1,
    );
    context.set_account(&FOUNDER_COLLATERAL, &AccountSharedData::from(account));

    let mut instructions = Vec::new();
    if let Some(approve) = plan.approve.clone() {
        instructions.push(approve);
    }
    instructions.push(plan.instruction.clone());
    let outcome = submit_hostile(
        &mut context,
        &instructions,
        "conservation hostile: a split without collateral",
    )
    .await;
    assert_eq!(
        outcome.refusal,
        Some(ClaimsConservationSbfErrorV1::Balances as u32),
        "a split whose stated prestate is not what the account holds refuses `Balances` \
         before the transfer, not after it; logs: {:?}",
        outcome.logs,
    );
}

/// A merge that offers the Market's failure escrow as the actor's own Position
/// refuses `Identity`.
///
/// On a refunding Market the failure coordinate is the ESCROW's and the holder
/// never touches it. An actor who names it as their own is claiming to be the
/// Market's own derived escrow owner, and the Position's recorded owner is what
/// refuses.
#[tokio::test]
async fn a_merge_that_names_the_failure_coordinate_as_its_own_refuses_identity() {
    let (world, mut context) = founded_and_open(
        FoundingShapeV1::Refunding,
        0,
        0,
        "conservation hostile: a merge naming the failure coordinate",
    )
    .await;
    let plan = plan(
        &mut context,
        &world,
        ClaimsConservationDirectionV1::Merge,
        SPLIT_SETS,
    )
    .await;
    // The request names the escrow as the actor's Position and the frame seats
    // it there, so the privilege pass -- which only compares the account's key
    // to the request -- passes it through to the conjunct under test.
    let named = ClaimsConservationRequestV1 {
        position: world.escrow_position.to_bytes(),
        ..plan.request
    };
    let mut accounts = plan.instruction.accounts.clone();
    *accounts
        .get_mut(frame_v1::POSITION)
        .expect("the holder coordinate") = AccountMeta::new(world.escrow_position, false);
    let hostile = Instruction {
        program_id: plan.instruction.program_id,
        accounts,
        data: named
            .to_bytes()
            .expect("canonical request naming the escrow as the holder")
            .to_vec(),
    };
    let outcome = submit_hostile(
        &mut context,
        &[hostile],
        "conservation hostile: a merge naming the failure coordinate",
    )
    .await;
    assert_eq!(
        outcome.refusal,
        Some(ClaimsConservationSbfErrorV1::Identity as u32),
        "the escrow's Position is the Market's, not the signer's, and the route says so by \
         name; logs: {:?}",
        outcome.logs,
    );
}

/// A merge of more complete sets than the holder holds refuses `Holding`, on a
/// CATEGORICAL Market.
///
/// The other shape, and the control that this route is not a refunding-only
/// one: a categorical Market's complete set lives in ONE Position, seats no
/// escrow, and pays at scale 1. The founding admitted `QUANTITY` sets and no
/// act has added one, so a merge of `QUANTITY + 1` asks the executor to debit
/// a coordinate that holds less than the quantity.
///
/// The stranger's donation is what makes the overreaching request
/// REPRESENTABLE: without it the vault could not cover `(QUANTITY + 1)` sets'
/// collateral and the contract would refuse the wire's arithmetic before the
/// route ever saw it -- which would prove something about `validate`, not about
/// the holding.
#[tokio::test]
async fn a_categorical_merge_of_an_incomplete_set_refuses_holding() {
    let (world, mut context) = founded_and_open(
        FoundingShapeV1::Categorical,
        0,
        DONATION_ATOMS,
        "conservation hostile: an incomplete categorical merge",
    )
    .await;
    donate_to_the_vault(
        &mut context,
        &world,
        DONATION_ATOMS,
        "conservation hostile: the donation that makes the overreach representable",
    )
    .await;
    let plan = plan(
        &mut context,
        &world,
        ClaimsConservationDirectionV1::Merge,
        QUANTITY,
    )
    .await;
    assert!(
        !plan.refunds_on_failure,
        "a categorical Market seats no escrow, and the record is what says so",
    );
    let scale = plan.request.basis_scale;
    let atoms = (QUANTITY + 1) * scale;
    let overreach = ClaimsConservationRequestV1 {
        quantity: QUANTITY + 1,
        collateral_atoms: atoms,
        post_hoard_amount: plan.request.pre_hoard_amount - atoms,
        post_external_amount: plan.request.pre_external_amount + atoms,
        ..plan.request
    };
    // The frame stays the plan's own. Raising the quantity moves the request's
    // digest and therefore the caller-authority PDA the Custody CPI would need,
    // but the executor runs over candidates BEFORE any collateral moves, so the
    // holding is what refuses and the stale authority is never read.
    let hostile = Instruction {
        program_id: plan.instruction.program_id,
        accounts: plan.instruction.accounts.clone(),
        data: overreach
            .to_bytes()
            .expect("canonical overreaching request")
            .to_vec(),
    };
    let outcome = submit_hostile(
        &mut context,
        &[hostile],
        "conservation hostile: an incomplete categorical merge",
    )
    .await;
    assert_eq!(
        outcome.refusal,
        Some(ClaimsConservationSbfErrorV1::Holding as u32),
        "a merge finds less than `quantity` at a coordinate it burns and says so; \
         logs: {:?}",
        outcome.logs,
    );
}

/// A split whose two token accounts are named in the opposite roles refuses
/// `Identity`.
///
/// The vault's seeds are DIRECTION-FREE: they come from the Market, the release
/// set, the custody context and the compartment, never from whichever side of
/// the transfer happens to be the source. An earlier draft derived them from the
/// request's source side, which on a split is the External side, so every split
/// would have refused here. This is that defect's guard on the real ELF, and the
/// route holds it twice -- the vault's owner must be the Custody transfer
/// authority, and its address must be the derived one. The first fires.
#[tokio::test]
async fn the_reversed_transfer_pair_refuses_identity() {
    let split_atoms = SPLIT_SETS * refunding_scale();
    let (world, mut context) = founded_and_open(
        FoundingShapeV1::Refunding,
        split_atoms,
        0,
        "conservation hostile: the reversed pair",
    )
    .await;
    let plan = plan(
        &mut context,
        &world,
        ClaimsConservationDirectionV1::Split,
        SPLIT_SETS,
    )
    .await;
    let reversed = ClaimsConservationRequestV1 {
        hoard_vault: FOUNDER_COLLATERAL.to_bytes(),
        external_collateral: plan.request.hoard_vault,
        ..plan.request
    };
    let mut accounts = plan.instruction.accounts.clone();
    *accounts
        .get_mut(frame_v1::HOARD_VAULT)
        .expect("the vault coordinate") = AccountMeta::new(FOUNDER_COLLATERAL, false);
    *accounts
        .get_mut(frame_v1::EXTERNAL_COLLATERAL)
        .expect("the external coordinate") = AccountMeta::new(plan.hoard_vault, false);
    let hostile = Instruction {
        program_id: plan.instruction.program_id,
        accounts,
        data: reversed
            .to_bytes()
            .expect("canonical reversed request")
            .to_vec(),
    };
    let outcome = submit_hostile(
        &mut context,
        &[hostile],
        "conservation hostile: the reversed pair",
    )
    .await;
    assert_eq!(
        outcome.refusal,
        Some(ClaimsConservationSbfErrorV1::Identity as u32),
        "the account offered as the vault is not owned by the Custody transfer authority; \
         logs: {:?}",
        outcome.logs,
    );
}

// ---------------------------------------------------------------------------
// The census's own control
// ---------------------------------------------------------------------------

/// The census reads VIOLATED when a law is broken, or its greens prove nothing.
///
/// Every other test in this file asserts that eight laws HOLD over a round
/// trip. An instrument that cannot report a breach reports the same thing
/// whether or not one occurred, so this drives one observation past each law
/// and requires the exact verdict.
#[test]
fn the_census_reads_red_when_a_law_is_broken() {
    let sound = |stage: &'static str| ObservationV1 {
        stage,
        mint_supply: 32,
        token_atoms: BTreeMap::from([("hoard", 21), ("founder", 6), ("stranger", 5)]),
        class_atoms: BTreeMap::from([("HoardPrincipal", 21)]),
        accounts: BTreeMap::from([("hoard", (true, 2_039_280))]),
        aggregate_supply: vec![7, 7, 7, 7],
        position_totals: vec![7, 7, 7, 7],
        claim_unit_atoms: 3,
        payer: Pubkey::new_from_array([0x31; 32]),
        payer_lamports: 10_000_000_000,
        declared_class_deltas: BTreeMap::from([("HoardPrincipal", 0)]),
        declared_collateral_delta: 0,
        declared_fee_lamports: Some(5_000),
        lamports_inapplicable: None,
    };
    let status = |census: &CensusV1, now: &ObservationV1, law: &str| -> &'static str {
        census
            .evaluate(now)
            .into_iter()
            .find(|verdict| verdict.law == law)
            .unwrap_or_else(|| panic!("{law} is evaluated"))
            .status
    };

    let mut census = CensusV1::default();
    let first = sound("first");
    for law in ["L1", "L3", "L4"] {
        assert_eq!(
            status(&census, &first, law),
            "holds",
            "{law} is stated over one observation and holds on a sound one",
        );
    }
    for law in ["L2", "L5", "L6", "L7", "L8"] {
        assert_eq!(
            status(&census, &first, law),
            "inapplicable",
            "{law} needs a predecessor and says so rather than reading green",
        );
    }
    census.observations.push(first);

    let mut untracked = sound("untracked");
    untracked.mint_supply = 33;
    assert_eq!(
        status(&census, &untracked, "L1"),
        "violated",
        "L1 sees an atom in an account the census does not name",
    );

    let mut undeclared = sound("undeclared");
    undeclared.token_atoms.insert("hoard", 26);
    undeclared.class_atoms.insert("HoardPrincipal", 26);
    undeclared.mint_supply = 37;
    assert_eq!(
        status(&census, &undeclared, "L2"),
        "violated",
        "L2 sees the Hoard move by an amount the stage did not declare",
    );
    assert_eq!(
        status(&census, &undeclared, "L8"),
        "violated",
        "and L8 sees the same movement as the HoardPrincipal class's",
    );

    let mut unbalanced = sound("unbalanced");
    unbalanced.position_totals = vec![7, 7, 7, 6];
    assert_eq!(
        status(&census, &unbalanced, "L3"),
        "violated",
        "L3 sees a claim the aggregate owes that no Position holds",
    );

    let mut thin = sound("thin");
    thin.token_atoms.insert("hoard", 20);
    thin.class_atoms.insert("HoardPrincipal", 20);
    thin.mint_supply = 31;
    assert_eq!(
        status(&census, &thin, "L4"),
        "violated",
        "L4 sees a vault that does not back the worst outcome at the basis scale",
    );

    let mut leaked = sound("leaked");
    leaked.token_atoms.insert("founder", 5);
    leaked.mint_supply = 31;
    assert_eq!(
        status(&census, &leaked, "L5"),
        "violated",
        "L5 sees the tracked total move when the stage declared it would not",
    );

    let mut closed = sound("closed");
    closed.accounts.insert("hoard", (false, 0));
    assert_eq!(
        status(&census, &closed, "L6"),
        "violated",
        "L6 sees a funded watched account vanish",
    );

    let mut overpaid = sound("overpaid");
    overpaid.payer_lamports = 10_000_000_000 - 6_000;
    assert_eq!(
        status(&census, &overpaid, "L7"),
        "violated",
        "L7 sees the payer lose more lamports than the fee it declared",
    );

    let mut excused = sound("excused");
    excused.payer_lamports = 10_000_000_000 - 6_000;
    excused.lamports_inapplicable = Some("a stated reason");
    assert_eq!(
        status(&census, &excused, "L7"),
        "inapplicable",
        "and an INAPPLICABLE names its reason rather than passing",
    );
}
