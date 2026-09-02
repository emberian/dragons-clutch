//! The compute margin of the public Direct route, asserted rather than hoped.
//!
//! # Why a gate and not a note in a report
//!
//! Wall #28 was ruled ACCEPT on 2026-08-30: the top-level Direct route passes
//! every seed measured, on a margin of a percent and a bit. Accepting a margin
//! that thin is only honest if something notices when it erodes, and the
//! evidence that it erodes silently is in this repository's own history.
//! `df404c56` grew Core by 7,520 CU -- and its commit message says "the record
//! type only, no route and no program change yet", because it changed two
//! SHARED contract crates and nobody expected a program to move. It was true
//! and it was still costly.
//!
//! So this file is the condition of that ruling. The next commit that quietly
//! costs eight thousand compute units goes red here, at its author, instead of
//! on devnet a month later with nobody able to say which change did it.
//!
//! # What this gate asserts, and why it stopped asserting the worst draw
//!
//! Until 2026-08-30 the assertion was `worst swept seed <= 1,387,000`, and that
//! number was a SAMPLE of a lottery, not a bound on anything. The measurement
//! that retired it is recorded in full in
//! `docs/evidence/DIRECT_HOT_CU_VARIANCE_CENSUS_2026-08-30.md`, and its one
//! sentence is this: between `ff543148` and `9dbbc371` the route's worst swept
//! seed fell 1,390,745 -> 1,363,745 and its cross-seed band collapsed 52,500 ->
//! 24,000, while the route's KEY-INDEPENDENT COST changed BY ONE COMPUTE UNIT.
//! The 27,000 was a redraw. A gate pinned to the worst draw would have called
//! that a 27,000 CU improvement and been wrong by 27,000 CU.
//!
//! What this file asserts instead is the part of the cost that belongs to the
//! CODE. Every compute unit on this route is one of exactly two things:
//!
//! * a key-independent constant -- call it `C0`, the cost with every remaining
//!   `find_program_address` landing on its first candidate; and
//! * 1,500 CU for every candidate a bump search rejects, summed over the SEVEN
//!   key-varying search sites censused below.
//!
//! `CU(seed) = C0 + 1,500 * T(seed)` holds to within 142 CU across 32 key draws
//! at `ff543148` and 33 CU at `9dbbc371`, and the sweep ASSERTS it rather than
//! remarking on it: every seed's residual must sit on the 1,500 CU grid, so the
//! day a key starts moving something that is not a bump search, the site census
//! below is known to be stale instead of quietly wrong. `C0` is a property of
//! the code alone; `T` is a property of the keys. So the gate is on `C0`, and
//! the ceiling question -- which is a question about keys -- is answered where
//! it belongs, as a probability, printed below and derived in the evidence doc.
//!
//! # The census was ten sites until the fixture stopped lying about the carry
//!
//! `a0cba859` gave Trading, Claims and Custody a `create_program_address` path
//! for the Market state and for Custody's realm record pair, reading the bumps
//! `CoreState` records at founding. The fixture staged
//! `StateBumpsV1::UNRECORDED` anyway -- `e93fe5e9` put that literal in to keep
//! the file compiling and nothing revisited it -- so all five readers took their
//! search fallback and every number this gate ever printed belonged to a market
//! no widened founding produces. The variance census found it; the fixture now
//! stages the bumps `plan_found` writes, and this gate ASSERTS that it does --
//! `search_depths` compares the staged tail against a re-derivation of it on
//! every seed, and `a_wrong_recorded_bump_refuses_instead_of_reaching_the_account_it_names`
//! bends each of the three and requires the refusal. So the carry cannot go
//! inert again, and cannot be decorative, without a red here.
//!
//! Measured across the same 32 seeds on the same five ELFs, the fix is worth
//! `9,000 + 4,500 * (market_depth - 1) - 84` CU on EVERY seed -- the realm pair
//! at its constant depth, the Market's one draw at its three sites, and 84 CU
//! the five carried readers spend reading the tail. Floor 1,323,242 ->
//! 1,318,826, worst 1,363,745 -> 1,345,829, band 24,000 -> 16,501.

use solana_account::AccountSharedData;
use solana_program::{instruction::InstructionError, pubkey::Pubkey};
use solana_program_test::BanksClientError;
use solana_sdk::{signature::Signer, transaction::TransactionError};

use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{CallerRoleV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1};
use dclutch_direct_codec::successor::{DirectCoordinatesV1, MakerReplaySeedsV1};
use dclutch_direct_hot_program_test_support::waist::{
    CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, DirectCase, REGISTRY_PROGRAM_ID, RefusedExecution,
    Releases, TRADING_PROGRAM_ID, add_lookup_table, add_release_waist, canonical_lookup_addresses,
    direct_case, direct_top_level_instructions, elves, fixture_substrate,
    program_test_without_forced_budget, start_with_substrate, submit_v0_observed,
    with_fixture_seed,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2};
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading_sbf::TradingSbfError;

/// Fixture seeds swept by the gate.
///
/// Twelve was the first answer and it was WRONG, in the direction that matters.
/// Twelve seeds put this route's worst draw at 1,373,917 CU; thirty-two put it
/// at 1,381,576. A gate pinned to the twelve-seed figure would have gone red on
/// seed 15 -- a legitimate key draw, not a regression -- and the first person to
/// meet that red would have learned to distrust this file.
///
/// Thirty-two is not magic either. It is enough draws for the band to stop
/// moving much, and the cost is about forty seconds.
///
/// # The lottery this sweep CANNOT see, which is not the maker's keys
///
/// Redrawing the fixture seed redraws the payer and the two makers. It does not
/// redraw the other input to every bump search on this route: `release_set_id`
/// is `hash(ExecutionReleaseSetV1)` over the deployed ELF DIGESTS, and it seeds
/// the activation cache directly and the Market identity transitively -- and
/// the Market PDA seeds the Claims market, the positions, the maker replays and
/// every caller authority downstream of them.
///
/// So changing any role's SOURCE redraws all of it, and two CU figures taken
/// from two different source trees are not comparable however careful the rest
/// of the method was. Measured, and this is the sharpest instance of it anyone
/// has recorded here: `557df0d1` split one stack frame in `direct_replay_setup_v1`
/// and moved this sweep's worst seed by 27,000 CU and its band by 28,500, having
/// changed the route's key-independent cost by 1 CU.
///
/// The older wording of this paragraph said a REBUILD redraws it "with no
/// source change at all". That is false and was worth checking: building the
/// five role ELFs twice from the same clean `ff543148` tree reproduced the whole
/// 32-seed sweep to the compute unit -- min 1,338,245, worst 1,390,745, mean
/// 1,355,639. The link is deterministic. What redraws the lottery is a source
/// change, anywhere in any of the five roles.
const GATE_SEEDS: u64 = 32;

/// 1,500 CU per candidate a bump search rejects, and per `create_program_address`.
///
/// `sol_try_find_program_address` charges `create_program_address_units` once up
/// front and again for every rejected candidate, so a search landing on bump
/// `b` costs `(256 - b) * 1,500`. This is the ONLY thing on this route whose
/// cost moves with a participant key.
const ATTEMPT_COST_CU: u64 = 1_500;

/// Attempts `find_program_address` makes to land on `bump`.
const fn attempts(bump: u8) -> u64 {
    256 - bump as u64
}

/// The seven key-varying search SITES this route still pays, over seven addresses.
///
/// Censused at `ff543148` and re-verified at `9dbbc371` by reading every reached
/// `find_program_address` on the route and then PROVING the list complete: the
/// residual `CU(seed) - 1,500 * T(seed)` is constant across 32 key draws to
/// within 142 CU, which it could not be if a site were missing.
///
/// | address | sites | seeded by |
/// |---|---|---|
/// | Direct capability root | 1 -- Trading `dispatch.rs:318` | the Market, plus the config record digest |
/// | seller maker replay | 1 -- Trading `hot_v3.rs:6292`, preplan pass | the seller key |
/// | buyer maker replay | 1 -- Trading `hot_v3.rs:6292`, preplan pass | the buyer key |
/// | Custody caller authority | 1 -- Trading `child_authority_v4.rs:65` | the projected child request digest |
/// | Claims caller authority | 1 -- Trading `claims_composition_v3.rs:162` | the parent request digest and the packet digest |
/// | Custody replay | 1 -- Custody `lib.rs:787` | the buyer's maker-replay root |
/// | Custody transfer authority | 1 -- Custody `lib.rs:1336` | the Market |
///
/// SIX of the seven are reproduced below from the fixture's own planted
/// accounts. The seventh -- the Claims caller authority -- is not, because its
/// packet digest is the only seed no public fixture field carries; it is left in
/// the residual on purpose, and the floor statistic below is exactly what that
/// costs when it lands on its first candidate.
///
/// # The three sites that left this table, and what they were worth
///
/// The Core Market state was **3** of the ten sites this census used to carry --
/// Trading `hot_v3.rs:10398`, Claims `sparse_native_transfer_v1.rs:518`, Custody
/// `lib.rs:509`, one draw read three times, the single largest term in the
/// variance. All three now reproduce the address from `CoreState`'s recorded
/// bump instead of searching, because the fixture stages the bump the founding
/// writes. Measured over these 32 seeds the Market drew depths 1-4, so the three
/// sites were worth `4,500 * (depth - 1)` CU: 0 on the luckiest draws and 13,500
/// on the unluckiest, and the band fell 24,000 -> 16,501 accordingly.
///
/// Custody's realm raw/staging pair left the same way and is not in this table
/// for a different reason: its seeds are `[domain, schema, content digest]` and
/// carry no participant key, so it was never key-varying -- it was a flat term
/// of `C0`, 12,000 CU at this deployment's depths and 3,000 now. That 9,000 is
/// the whole reason the floor below moved at all; see its arithmetic.
const KEY_VARYING_SEARCH_SITES_V1: u64 = 7;

/// The key-independent cost of this route, as the sweep can observe it.
///
/// The statistic asserted is `min over seeds of (CU(seed) - 1,500 * T_known(seed))`,
/// where `T_known` is the attempt count of the six modelled sites. That
/// minimum equals `C0 + 1,500 * k` where `k` is the seventh site's attempt count
/// on the luckiest of the thirty-two draws, and `k = 1` unless all thirty-two
/// draws missed on their first candidate -- probability `2^-32`. So this is a
/// BOUND on a property of the code, not a sample of a lottery: no key draw can
/// move it, and two of the thirty-two seeds attain it.
///
/// # The arithmetic, at decision 0017 option B with the founding bumps staged
///
/// Measured floor statistic 1,252,751, so `C0` = 1,251,251 and a route whose
/// seven searches all landed first try would cost `C0 + 7 * 1,500` = 1,261,751.
/// The constant below is the measured floor plus 1,500 -- exactly one bump
/// attempt, the smallest unit this route is capable of spending. A change that
/// costs less than a single PDA search does not go red here; anything that costs
/// a search or more does, and `df404c56`'s 7,520 CU would have gone red five
/// times over.
///
/// # Why the floor fell 66,921 CU, and why THIS one is margin somebody won
///
/// Decision 0017's option B: the top-level arm stopped invoking
/// `RegistryInstructionV1::Reauthenticate` twice and read the activation cache
/// itself. Measured on the same 32 seeds and the same method, 1,319,672 ->
/// 1,252,751, and it decomposes into two terms that were separately predicted:
///
/// ```text
///   the two Registry CPIs, SEALWIDE's measured 26,296 each               -52,592
///   the third full cache decode -- 25 `decode_role` calls -- that
///     `authenticate_activated_child_programs_v3` paid to learn two
///     program ids the caller's own decode already held                  -c. 14,300
///   the local replacement: two role decodes, two deployment
///     observations, one identity check                                   +c. 3,000
///   ------------------------------------------------------------------ -66,921
/// ```
///
/// Unlike the 4,416 above, none of this moved from the key-varying term into
/// `C0`: the CPI cost contained no bump search of its own that this route does
/// not still pay, so the site census below is unchanged at seven and the whole
/// 66,921 is margin the route did not have before. `docs/design/TRUST_RATCHET_V1.md`
/// §8.2 asked for exactly this number at exactly this statistic; its arithmetic
/// net of ~49,500 was low, because it sized the CPI pair and not the decode the
/// pair made redundant.
///
/// # Why the floor moved 4,416 CU and why that is NOT margin anybody won
///
/// The old statistic was 1,323,242 over ten sites. Staging the founding's bumps
/// moved it three ways at once, and only one of the three is a saving:
///
/// ```text
///   Custody's realm raw/staging pair, 8 attempts -> 2 create_program_address   -9,000
///   the three Market readers' first attempt, formerly subtracted with the
///     site and now a flat part of C0                                          +4,500
///   the five carried readers reading the CoreState tail, measured             +   84
///   ------------------------------------------------------------------------ -4,416
/// ```
///
/// So this constant fell because a CONSTANT cost was removed. The 4,500 did not
/// go anywhere; it moved from the key-varying term into `C0`, which is exactly
/// what a carry does and exactly why the site count is the deliverable. What the
/// route actually won is in the band and the tail, not here.
///
/// It is not the protocol ceiling and it must never be raised to meet a
/// regression. Raising it IS the act of spending margin, and it should cost a
/// decision and a sentence saying what got cheaper in exchange.
///
/// # What this gate does NOT do, stated because it would otherwise be assumed
///
/// It does not say the route fits. It says the code's constant part has not
/// grown. Whether an arbitrary stranger's trade fits is a question about a
/// GEOMETRIC DISTRIBUTION, and the sweep prints the answer rather than asserting
/// it: with `C0` at 1,317,326 and seven searches whose depth is `Geometric(1/2)`,
/// `P(CU > 1,400,000)` is 0.0000001% -- about one public trade in 1.1 billion,
/// down from one in 3,100 when the Market still searched at three sites. At the
/// conservative fitted `p̂ = 0.446` it is one in 13.9 million, down from one in
/// 629. No gate constant can make that figure untrue while the searches remain,
/// which is why the site count above is the real deliverable and the constant is
/// only the regression detector.
/// # Why it rose 8,874 CU on 2026-08-31, and who moved each part of it
///
/// This is the act the paragraph above calls spending margin, so it is spent
/// with an itemisation rather than a shrug. The same gate, by the same method,
/// on three trees:
///
/// ```text
///   this constant, as recorded                                    1,254,251
///   the fee lane's base `a0b1f4cb`, before any of its own work     1,259,047   +4,796
///   the fee lane merged with main at `59ecec5f`                    1,263,125   +4,078
///   ------------------------------------------------------------------------ +8,874
/// ```
///
/// **+4,796 is the fee-core protocol tier**: `DirectMakerReplayLayoutV1` widened
/// 152 -> 160 for `fee_owed`, and the fee-band `require` in the transition.
/// Neither had ever been run against an on-chain gate, and that branch was
/// already red here before the second-transaction lane touched it.
///
/// **+4,078 is the second-transaction lane plus main's own drift**: one
/// `write_u64` per maker replay in the Effect -- without which `fee_owed` is
/// permanently zero on chain -- the eight extra record bytes hashed on both
/// sides of the poststate comparison, and the poststate projection's delegation
/// branch.
///
/// Nothing got cheaper in exchange and this comment's bargain is not met. What
/// is offered instead is that a fee-bearing Direct market now EXISTS: it was
/// unreachable at any price before, over the ceiling by more than the whole fee
/// leg cost. 8,874 CU buys it, against 105,373 CU of remaining worst-seed
/// margin on this same sweep. That is a judgement and it is recorded as one.
/// Evidence: `docs/evidence/FEE_SECOND_TRANSACTION_PAIR_2026_08_31.md`.
///
/// # Why it rose a further 1,551 CU on 2026-08-31, and why 1,500 of that is slack
///
/// The constant above was set to a MEASURED FLOOR WITH ZERO HEADROOM at 02:06,
/// and the tree it was measured on was merged with main at 02:16. Ten minutes.
/// The cohort-8 cut measured the merged tree and read 1,263,176 -- fifty-one
/// compute units over a pin that had no room for one. Same gate, same method:
///
/// ```text
///   the fee lane's own commit `28530782`, re-measured here   1,263,125     0
///   the cohort-8 cut candidate `dfb41be6`                    1,263,176   +51
/// ```
///
/// The first line REPRODUCES the recorded constant exactly, which is what makes
/// the second line a measurement of drift rather than of a different instrument.
///
/// **The +51 is main's own drift, merged into the lane after the pin was set.**
/// Eliminated by inspection: the final merge `f8cf60cc` -> `a7d50d3a` added no
/// Rust at all under `programs`/`crates` (two shell scripts); `8b47f287` touched
/// only `tools/gauntlet`; and NO `Cargo.lock` changed in either merge range, so
/// it is not dependency drift. What remains is the set merged at `ab428f63`,
/// of which only these touch non-test code this route can reach --
/// `d38aadae` (`dclutch-claims-svm`, and the Claims caller authority is one of
/// the seven sites), `aac98afd` (`dclutch-product-payoff-v2-codec`),
/// `f3f47640` (`dclutch-token-svm`). Per-commit attribution is queued; the
/// honest statement today is 51 CU, measured, from that set.
///
/// **The 1,500 is deliberate slack and it is the actual lesson here.** A pin set
/// to its own floor reddens on the next commit that costs a single unit, which
/// is what happened, and a gate that cries at 51 CU while the route sits 108,322
/// CU under the ceiling trains its readers to raise it without looking. 1,500 is
/// one whole bump-search attempt -- the quantum this route's cost actually moves
/// in -- about thirty times the drift just measured, and far below the class of
/// change this gate exists to catch: `df404c56` cost 7,520 CU while believing it
/// had changed no program at all. Routine drift will not re-redden this; a real
/// regression still will. It also restores the headroom FIXBUMPS chose, which
/// this file's own history records as "exactly 1,500 CU".
///
/// For the record, the route got CHEAPER, not dearer: cohort-7 shipped a
/// 1,319,583 floor and this is 1,263,176, 56,407 CU below it, with the
/// affordable bump-search count up from 54 to 85 over the same seven sites.
///
/// # Why it rose 6,876 CU on 2026-08-31, and why this one is a REGRESSION
///
/// The two paragraphs above spend margin and say what was bought. This one
/// spends margin and the honest answer is that nothing was bought. It is
/// recorded that way on purpose.
///
/// The method first, because it is what makes the rest quotable. The same gate,
/// same statistic, same 32 seeds, each revision built as a clean `git archive`
/// with its own ELF set. At the commit that set the constant above the sweep
/// reproduced it TO THE UNIT -- `1743be0a` reads 1,263,176, and the gate passes
/// there. A bisect that cannot reproduce the old number is measuring a different
/// instrument, so that control comes first and everything below is a difference
/// against it.
///
/// ```text
///   the pin, 1743be0a                                          1,263,176
///   main to b3e3821c                                                +343
///   main to a18760c4                                              +1,330
///   the basis lane merging in                                     +5,058
///   the Claims 49->50 account frame, and its rent-credit auth          +2
///   main to 612ddeb3                                                +144
///   ------------------------------------------------------------------
///   measured here                                              1,270,053
///   the clippy regrouping in `retirement_checkpoint_v1`                -1
///   ------------------------------------------------------------------
///   the tree this constant is pinned to                        1,270,052
/// ```
///
/// **74% of it is one lane, and it is not the one anybody suspected.** The
/// candidates offered were CLOSEMAKER's fifth `ProgramSet` entry, SPLINE-WIRE's
/// two optional `Found` accounts, and the union'd refusal registries. All three
/// are refuted. `b3e3821c` sits after the first two and the whole span to it
/// costs 343 CU; the registries are `const _: () = { ... }` and `pub const ALL`,
/// which no trade evaluates. The Claims frame growing 49 -> 50 accounts -- the
/// most trade-shaped change in the range -- costs **2 CU**, because
/// `process_fractional_compaction` is behind a magic-byte dispatch a Direct fill
/// does not carry, and the CPI account list on this route is built from
/// `invocation.fixed_account_count`, not from that frame constant.
///
/// **What it actually is**: `authenticate_product_basis_v3` in
/// `dclutch-product-runtime-v2-svm-reader`. Split against its own merge base
/// `2ebeb798` (1,263,174, which is the pin's value to within 2 CU, the two
/// branches having diverged there):
///
/// ```text
///   48d591b5 + ae173f02 + b951bba2, admit_selection_v3 made
///     unconditional on the shared join                              +446
///   dbf5fe02 + 5ef8a221 + fcd6aecc, ProductBasisV3::decode
///     rewritten and the price-gate digest probe added             +4,567
///   ------------------------------------------------------------------
///   the lane                                                      +5,013
/// ```
///
/// The gate is green at `b951bba2` and red at `fcd6aecc`, so the second line is
/// where the public route lost its margin. The multiplicity is why three small
/// reads add up: `authenticate_product_basis_v3` runs TWICE per transaction --
/// Trading `hot_v3.rs:1999` and the Claims child `sparse_native_transfer_v1.rs`
/// -- and `ProductBasisV3::decode` runs twice inside each, so the rewritten
/// decode executes FOUR times per trade and the digest probe twice.
///
/// **The design said this would cost nothing, and that claim is now false.**
/// `docs/design/BASIS_ABI_UNIFICATION_V1.md` line 735 reads "No trade ever
/// verifies a certificate, and the hot path gains exactly zero CU", and its
/// hostile 21 says the hot path's cost is "asserted by CU measurement". The
/// first half is true and this sweep confirms it -- the fixture stages a zero
/// `price_gate_certificate_digest` and `verify_price_gate_v1` never runs. The
/// second half was never measured against this gate. The comment at
/// `svm-reader/src/lib.rs:459` calls that function "founding's join, the place
/// Core reaches before it commits a founding permit", and that premise is what
/// is wrong: it is also the join the public Direct route takes, twice.
///
/// **This comment's bargain is NOT met and no trade was made.** The 8,874 above
/// bought a fee-bearing market that was unreachable at any price. This 6,876
/// buys an admission check on a route that does not need it and a probe for a
/// gate that cannot fire at degree <= 1. It is pinned rather than left red
/// because a red gate nobody can act on stops being read, and because the cost
/// is now attributed to the line -- but the margin is spent and it is spent on
/// nothing.
///
/// The fix is cheap and belongs to the basis lane, not here: hoist
/// `admit_selection_v3` and the certificate-digest probe to the founding caller,
/// or gate them on the founding path, and roughly 4,500 CU comes back to the
/// public route. Until then this route sits 129,948 CU under the ceiling on the
/// key-independent statistic, against 105,373 when the fee leg landed.
///
/// # 2026-08-31, recovered: 1,270,052 -> 1,266,559
///
/// The founding-only admission and certificate-digest probe now run only at
/// Core's permit-committing callers. Trading and Claims still authenticate the
/// immutable Registry record and all Product joins on every use, but consume
/// the semantic preimage from the already-decoded `ProductBasisV3` instead of
/// decoding the same bytes again. The paired fee gate moved by the same amount
/// to within 3 CU: 1,269,919 -> 1,266,429.
///
/// Measured recovery is **3,493 CU**, not the correction note's approximate
/// 4,500. The remaining cost is the one full canonical basis decode each of the
/// two independently authenticating programs must still perform; removing that
/// would weaken per-use authentication rather than hoist a founding conjunct.
/// The gate keeps its one-search-attempt slack, so the pin is measured floor
/// 1,266,559 + 1,500.
///
/// # 2026-09-01: measured 1,293,025, and DELIBERATELY NOT PINNED
///
/// The overnight completion wave -- 296 commits, `5b6a5849..371409f4` -- moved
/// this statistic **+26,466 CU**. Not one commit in it measured this gate and
/// `docs/evidence/` holds no CU record for the wave. The constant stays where
/// it is, RED, because every entry above records margin spent on a purchase it
/// can name, and raising it here would file an unmeasured wave as a settled
/// cost. This entry is the conviction the tier's own message asks for --
/// "find what got more expensive before touching its constant" -- not a
/// re-pin request.
///
/// Method, identical at both ends and the one this file already asserts: five
/// role ELFs built from the revision under test, 32 seeds, default substrate,
/// `min over seeds of (CU - 1500 * modelled attempts)`. `5b6a5849` reproduces
/// its own 1,266,559 TO THE UNIT and is green there, which is what makes the
/// far end of the comparison worth reading.
///
/// ## The commit, bisected by measurement
///
/// `6e91863c` *"trading: execute authenticated funding lifecycle"* --
/// **1,268,000 -> 1,278,367, +10,367 CU in one commit**, its parent `7783c31a`
/// green at 1,268,000 and it red. 1,176 inserted lines into `hot_v3.rs` under a
/// one-line message.
///
/// The remaining +14,276 is NOT convicted to a commit, and the reason is worth
/// more than the number. Above `bacd69ee` this gate cannot be run at all.
/// NINE revisions were sampled between `48d1f449` (08-31 23:40) and
/// `e0ece22e` (09-01 03:07) and every one of them failed: seven refuse the
/// public trade on **32 of 32 seeds** with `Custom(16387)`
/// (`TradingSbfError::Content`) -- `48d1f449`, `fd5d20ea`, `7ef13bd4`,
/// `a82316a8`, `5fbe0bd8`, `4f3d275e`, `7fe577e2`, `e0ece22e` -- and two do not
/// compile at all (`89bd7df2`, `E0432` in `dclutch-direct-codec`; `2d871068`
/// and its two neighbours will not build the program-test harness). The route
/// executes again only at `371409f4`. So for most of the wave the public Direct
/// trade was not expensive, it was BROKEN, and this gate's silence over that
/// span is the absence of a measurement rather than the absence of a change.
///
/// ```text
///   5b6a5849  1,266,559  green      7783c31a  1,268,000  green
///   7ca0210b  1,267,449  green      6e91863c  1,278,367  RED  <- +10,367
///   aca13a76  1,267,839  green      bacd69ee  1,278,749  RED
///   9031a8df  1,268,000  green      371409f4  1,293,025  RED  <- +14,276 more
/// ```
///
/// ## Where it went, by phase
///
/// This file's residual statistic taken PER PHASE over the `hot-cu-profile`
/// checkpoints: the minimum over the 32 seeds of each checkpoint interval, so a
/// phase's own bump draws cancel the way the total's do. One seed will not do
/// it -- on a single draw `commit-lifecycle-closes` reads -5,041 and
/// `request-lifecycle-preplan` +3,028, and both are the lottery, not the code.
/// The per-phase floors sum to +26,452, this gate's +26,466 to within 14 CU:
///
/// ```text
///   commit-non-root            117,212 -> 125,514   +8,302
///   p7-effect-projection       165,777 -> 172,731   +6,954
///   p5r-account-projection     138,583 -> 142,274   +3,691
///   artifacts-strategy-effect   56,480 ->  59,730   +3,250
///   commit-lifecycle-closes    351,380 -> 353,841   +2,460
///   twenty-nine other phases                        +1,795
/// ```
///
/// `6e91863c` is the only commit in its window to touch either of the top two:
/// it threaded a funding-V5 arm through `project_hot_effects_v3` and
/// `commit_non_root_effects_into_v3`. The obvious mechanism is REFUTED --
/// `SelectedEffectProgramV4::funding` is a `const fn` field read, not the
/// repeated decode the `basis:` entry above convicted -- so what is left is the
/// widened value threaded through the shared projection and commit walks, and
/// naming it exactly needs its own measurement that this lane did not take.
///
/// Profiling overhead is subtracted, not assumed: the instrumented ELF's own
/// floor sits 20,170 CU above the clean one at `371409f4` and 20,546 at
/// `5b6a5849`, over 35 checkpoints and 34 heap marks. The arithmetic checks
/// against the route -- `p7-local-effect-discipline` brackets no code, as its
/// own comment at `hot_v3.rs` says, and measures -2 CU net.
///
/// ## What it cost
///
/// Worst swept seed's margin 100,437 -> 75,472; analytic worst-case headroom
/// 59,941 -> 46,975. `direct_hot_fee_bearing_margin_gate` moved by the SAME
/// +26,466 (1,266,429 -> 1,292,895) with the two arms' 130 CU gap intact to the
/// unit, which is that file's own stated test for a cause upstream of fee work.
///
/// And it took the Hot continuation down:
/// `registry_hot_continuation::real_registry_executes_profile14_direct_hot_under_protocol_limit`
/// executes at `5b6a5849` with 8,492 CU to spare -- 1,391,358 of 1,399,850 --
/// and exhausts the meter at `371409f4`. That red is DOWNSTREAM of this one and
/// is not a second defect: +26,466 is three times the headroom it had.
/// # 2026-09-02: a further +6,233, bisected to a commit that changed no executed code
///
/// The entry above closes at `371409f4`. This statistic moved again after it --
/// 1,292,895 -> 1,299,128 on the fee-bearing arm, +6,233 -- and the bisect of
/// that increment is the reason this section is longer than the number
/// deserves.
///
/// Method identical to the entry above and to this file's own assertion: five
/// role ELFs built from the revision under test in a private worktree, 32 seeds
/// per arm, `min over seeds of (CU - 1500 * modelled attempts)`. The window is
/// the 54 commits in `371409f4..HEAD` that can change the Trading link, and the
/// far end reproduces: `5b6a5849` reads its own 1,266,429 TO THE UNIT.
///
/// ```text
///   371409f4  1,292,895   5ec149fa  1,293,701   9bdd1885  1,294,278
///   397ef013  1,294,370   0ba29756  1,294,312   cbdecdb3  1,294,312  green
///   5de38ef2  1,299,148  RED  <- +4,836 of the +6,233 in ONE commit
/// ```
///
/// ## The commit changed nine lines, and eight of them are comments
///
/// `5de38ef2` *"hot: one checkpoint splits the candidate phase"* adds nine lines
/// to `hot_v3.rs` and nothing else anywhere in the Trading link closure. Eight
/// are a comment. The ninth is `hot_cu_checkpoint!("candidate-transcript")`,
/// whose non-feature arm is `($phase:literal) => {};` -- it expands to nothing
/// -- and it sits inside `execute_admitted_candidate_v3`, which the public
/// Direct top-level fill does not call. Its message says "the macro is empty in
/// every shipped build". That is true of the MACRO and false of the LINK.
///
/// The Trading ELF is 104 bytes larger and `.text`, `.rodata`, `.rel.dyn`,
/// `.data.rel.ro` and `.dynamic` all differ; only `.dynstr`, `.dynsym` and
/// `.shstrtab` are identical. The other four role ELFs are byte-identical
/// across the pair, so the whole difference is Trading's.
///
/// **Positive control, because "the compiler is nondeterministic" is the
/// obvious escape and it is wrong here.** `cbdecdb3`'s Trading ELF was rebuilt
/// from a forced recompile in the same worktree and reproduced
/// `bfd447d509756b1d...` byte for byte. The build is deterministic; the
/// difference between the two revisions is real.
///
/// ## What that falsifies, which is worth more than the 4,836
///
/// The assertion below says this number "does not move when the keys or the
/// bump depths move, so this red is a CODE change and nothing else". Across
/// this pair it moved 4,836 CU with **both arms moving identically**, the
/// fee-bearing arm's 131 CU gap intact to the unit, and the modelled search
/// structure IDENTICAL on both sides -- seven distinct searched addresses over
/// seven search instances, `all-first-try` moving by exactly the same 4,836.
/// So the statistic carries a residual key-dependence its model does not
/// capture: something whose cost follows the ELF DIGEST -- `release_set_id`
/// hashes the five role ELFs and seeds every capability address under them --
/// and that is not one of the seven searches the model counts.
///
/// **A red here is a code change OR a relink, and telling them apart costs one
/// build.** Compare the five ELF digests across the pair first; if only the
/// section layout moved, the number moved without anything being spent.
///
/// NOT PINNED, for that reason and the one the entry above gives. `5de38ef2` is
/// convicted of MOVING this number and expressly not convicted of SPENDING it,
/// and a constant raised over a movement whose mechanism is unnamed files an
/// artifact as a settled cost. The remaining ~1,400 of the +6,233 is spread
/// across the rest of the window and is not convicted to anything.
///
/// ### RETRACTION, same day: the paragraph above blames the wrong thing
///
/// The two paragraphs above infer that this statistic carries a residual
/// dependence on the ELF DIGEST. **A control refutes that inference**, and it
/// is recorded here rather than quietly fixed because the inference is what
/// sent the next reader looking in the wrong place.
///
/// The control is a genuine relink: two five-ELF sets differing in Trading
/// alone, the other four byte-identical, across a real source change
/// (`2c51ecd1`, which restructures `direct_fee_settlement_v1` and touches
/// nothing the fill executes). Every modelled bump depth redrew -- at seed 0
/// the root went 3 -> 1, the two maker replays 1,4 -> 4,1, the Custody replay
/// 1 -> 3 -- and the activation cache went 254 -> 255.
///
/// ```text
///   floor, zero-fee      1,299,259 -> 1,299,276   +17
///   floor, fee-bearing   1,299,128 -> 1,299,145   +17
///   the 131 CU gap between the arms                 intact
/// ```
///
/// **Seventeen compute units.** So the statistic IS key-independent across a
/// relink to within 17 CU, and the assertion below stands as written.
///
/// Two consequences. First, THE ACTIVATION CACHE IS CLEARED: its depth changed
/// by a whole attempt across this pair and cost 17 CU, not 1,500, because the
/// Registry records that bump and the six readers reproduce the address with
/// `create_program_address` instead of searching (`waist.rs`, and the census in
/// `direct_hot_pda_depth_census.rs` says the same). A fix that added it to the
/// model was written against this file and REVERTED before landing once the
/// carried bump was read; it would have subtracted a cost nobody pays.
///
/// Second, `5de38ef2`'s +4,836 is therefore very likely REAL. That makes it
/// more interesting, not less: nine lines, eight of them comments, and the
/// ninth a macro whose non-feature arm expands to nothing, inside
/// `execute_admitted_candidate_v3` -- which the public fill does not call. This
/// route is documented as inlining-sensitive to exactly that magnitude at
/// `hot_v3.rs`, where extracting one function's checks out of line took the
/// canonical trade from passing to `consumed 1399850 of 1399850`.
///
/// **The next experiment, named so it is not re-derived a fourth time:** build
/// Trading at `cbdecdb3` and at `5de38ef2` and diff the SYMBOL TABLE, not the
/// ELF digest -- per-symbol sizes and stack frames over the functions the fill
/// actually executes. If those symbols are identical the CU cannot differ and
/// the measurement is what is wrong; if they differ, codegen moved and the
/// 4,836 is a recoverable accident rather than a conjunct the route needed.
/// Still NOT PINNED until that says which.
///
/// ### The experiment was run, and it says THE MEASUREMENT
///
/// Trading built at both revisions with `-Zemit-stack-sizes --emit=obj,link`.
/// Both links reproduce the digests the CU runs were taken on -- `bfd447d5...`
/// and `4e5dd776...` -- so the objects measured are the objects executed, and
/// both carry the same 3 frame diagnostics.
///
/// ```text
///   FUNC symbols            948  vs  948     differing sizes: 0
///   stack frames            941  vs  941     differing frames: 0
///   total code bytes  1,848,096  vs  1,848,096            +0
/// ```
///
/// **Not one function changed size and not one changed frame.** Codegen did not
/// move, so the executed instruction stream is the same and 4,836 CU of real
/// cost is impossible. The `.text` bytes DO differ, and that is consistent:
/// eight comment lines shift every later line number in `hot_v3.rs`, which moves
/// the panic-location strings in `.rodata`, which moves the addresses those
/// instructions carry as immediates. Same instructions, different immediates,
/// identical cost.
///
/// ### So the instrument is this file's own floor, and here is the defect
///
/// The floor is `min over 32 seeds of (CU - 1500 * modelled attempts)`. Differenced
/// SEED BY SEED across the two builds, the residual does not move by a constant:
///
/// ```text
///   fee-bearing   +1,835 .. +9,335    values 1500k + ~335
///   zero-fee      -8,661 .. +10,837   BOTH SIGNS
/// ```
///
/// Every unmodelled search is reseeded by `release_set_id`, so a relink
/// RESAMPLES the whole distribution. The floor is then an ORDER STATISTIC over a
/// resampled lottery -- the minimum of 32 fresh draws -- and the minimum of a
/// resampled distribution moves even when nothing it measures does. That is the
/// residual dependence, and it is not any one search's constant depth: it is the
/// `min` itself.
///
/// This partially un-retracts the section above. Its 17 CU control is sound and
/// still worth reading, but 17 CU is ONE SAMPLE of that order statistic, not a
/// proof of key-independence; this pair drew 4,836 from the same process.
///
/// **What would make the assertion true.** Either leave no unmodelled search in
/// the residual -- the Claims caller authority is the known one, and the fixture
/// already derives it at `direct-hot/src/fixture.rs` and discards the bump, so
/// exposing it is a contained change -- or stop calling a minimum a constant and
/// carry its sampling spread, comparing floors only within one link.
///
/// STILL NOT PINNED. A number is not a code cost until the statistic that reads
/// it has no lottery left in it.
///
/// ### Three relink pairs, and the lottery bites on one of them
///
/// The order-statistic reading predicts that a relink usually moves this number
/// by nothing and occasionally moves it by thousands. Measured, on three pairs
/// that differ in the Trading ELF alone:
///
/// ```text
///   real code change, direct_fee_settlement_v1 only      +17 CU
///   comment-only, 40 lines prepended to that same file     0 CU  (zero-fee +1)
///   comment-only, cbdecdb3 -> 5de38ef2                 +4,836 CU
/// ```
///
/// The middle pair is the control this section needed and it was built on
/// purpose: forty comment lines prepended to one module, which moves the ELF
/// digest and nothing else, both builds at zero frame diagnostics. It moved the
/// fee-bearing floor by ZERO. So a relink is not reliably expensive and is not
/// reliably free -- it is a draw, and the third pair drew badly.
///
/// That is why the fix is to the STATISTIC and not to the route. There is no
/// code in `5de38ef2` to make robust: 948 symbols and 941 frames say so. What
/// needs repair is a residual that still contains a search nobody subtracts.
const TOP_LEVEL_KEY_INDEPENDENT_CU_V1: u64 = 1_268_059;

/// The protocol maximum a transaction may consume.
const PROTOCOL_CEILING: u64 = 1_400_000;

/// `tools/gauntlet/CU_BUDGETS.json`'s tolerance, for the band this sweep saw.
///
/// The rule is `roundup(band, 10000) + 10000`, floor 15,000, and a budget of
/// `measured + tolerance` above the ceiling is that file "saying out loud that
/// a transaction has stopped fitting". The sweep prints the verdict rather than
/// asserting it: the band is a draw, so a red row here would be a lottery
/// result wearing the clothes of a code review.
fn cu_budgets_tolerance(band: u64) -> u64 {
    let rounded = band.div_ceil(10_000).saturating_mul(10_000);
    rounded.saturating_add(10_000).max(15_000)
}

/// The six modelled key-varying search sites, at one fixture draw.
#[derive(Clone, Copy, Debug)]
struct SearchDepthsV1 {
    /// Not a modelled site any more: all three Market readers reproduce this
    /// address from the recorded bump. Kept and printed because it is the
    /// measurement that says what the carry was worth on this draw -- three
    /// sites at `4,500 * (market - 1)` CU -- and because a reader who only sees
    /// the site table cannot tell a converted search from one nobody found.
    market: u64,
    root: u64,
    seller_replay: u64,
    buyer_replay: u64,
    custody_caller_authority: u64,
    custody_replay: u64,
    custody_transfer_authority: u64,
}

impl SearchDepthsV1 {
    /// Attempts across the six modelled sites. The Market is NOT among them.
    const fn attempts(self) -> u64 {
        self.root
            + self.seller_replay
            + self.buyer_replay
            + self.custody_caller_authority
            + self.custody_replay
            + self.custody_transfer_authority
    }
}

/// The bumps a founded Market's `CoreState` carries, and their canonical values.
///
/// Read back from the planted state and re-derived independently, because the
/// site census above is only true while the carry is live. `e93fe5e9` staged
/// `StateBumpsV1::UNRECORDED` here for eight months and every reader silently
/// took its search fallback; the cost of that was not a red test, it was six
/// weeks of compute numbers describing a market nobody deploys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CarriedBumpsV1 {
    market: Option<u8>,
    realm_raw: Option<u8>,
    realm_staging: Option<u8>,
}

/// One finalized record's canonical bump under this fixture's Registry.
///
/// The seed tuple is NOT restated here, which is the seam-audit rule
/// (`DOMAIN_RAW_RESTATEMENT`): `dclutch-record-contract` owns these domains and
/// exports the constructors that place them, so a crate that only READS the
/// address takes the domain from `seeds.domain()` instead of naming it. Two
/// spellings are two sources of truth, and the audit exists because the last
/// time two drifted apart nobody found out until an address stopped resolving.
fn record_bump_v1(seeds: RecordPdaSeedsV1) -> u8 {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        &REGISTRY_PROGRAM_ID,
    )
    .1
}

/// What the founding would have recorded, derived the way `plan_found` derives it.
fn canonical_bumps(core: CoreState) -> CarriedBumpsV1 {
    // The Realm record the founding authenticated: this Market's own
    // `realm_id` IS the content digest of its bytes, which is what
    // `authenticate_content_addressed_record` hashes and keys the pair on.
    let realm = RecordKeyV1::new(
        SchemaReleaseId::new(REALM_SCHEMA_RELEASE_ID_V1)
            .expect("the Realm schema release is nonzero"),
        ContentDigest::new(core.identity.realm_id.to_bytes())
            .expect("a founded Market names a nonzero Realm digest"),
    );
    CarriedBumpsV1 {
        market: Some(
            Pubkey::find_program_address(
                &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
                &CORE_PROGRAM_ID,
            )
            .1,
        ),
        realm_raw: Some(record_bump_v1(realm.raw_record_pda_seeds())),
        realm_staging: Some(record_bump_v1(realm.staging_cursor_pda_seeds())),
    }
}

/// What the fixture actually staged.
fn staged_bumps(core: CoreState) -> CarriedBumpsV1 {
    CarriedBumpsV1 {
        market: core.bumps.market,
        realm_raw: core.bumps.realm_raw_record,
        realm_staging: core.bumps.realm_staging_record,
    }
}

/// Reproduce the nine modelled searches from the fixture's own planted state.
///
/// Every seed tuple is read out of an account the fixture installed, never
/// restated from the fixture's private constants: the Market identity comes
/// from the `CoreState` bytes, the root's seeds from the root header's own
/// `seeds()`, and each derived address is checked against the address the
/// fixture reports. A model that addressed something else would still produce
/// a tidy number, so the equality checks are the point.
fn search_depths(direct: &DirectCase, releases: Releases) -> SearchDepthsV1 {
    let chain = &direct.chain;
    let data_of = |key: Pubkey| -> Vec<u8> {
        chain
            .accounts
            .iter()
            .find(|installed| installed.key == key)
            .map(|installed| installed.account.data.clone())
            .unwrap_or_default()
    };

    let core = CoreState::decode(&data_of(chain.market)).expect("the fixture plants a Core state");
    let (market_key, market_bump) = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        &CORE_PROGRAM_ID,
    );
    assert_eq!(
        market_key, chain.market,
        "the Market identity read out of the planted CoreState does not reproduce the Market \
         address the fixture reports, so this model is measuring a different account",
    );

    // The site census is a claim about which readers still search, and it is
    // only true while the state carries the bumps a founding records. Asserted
    // here, per seed, because it went silently false once already.
    assert_eq!(
        staged_bumps(core),
        canonical_bumps(core),
        "the planted CoreState does not carry the bumps `plan_found` records. If they are \
         `None` the CoreState carry is INERT again: all three Market readers and Custody's \
         realm pair take their search fallback, this file's seven-site census is wrong by \
         three sites, and TOP_LEVEL_KEY_INDEPENDENT_CU_V1 is measuring a market no widened \
         founding produces. Fix `build_direct_hot_chain_fixture_v5`, not this assertion.",
    );

    let root_bytes = data_of(chain.root);
    let header = CapabilityRootHeaderV1::decode(
        root_bytes
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .expect("the fixture plants a capability root header"),
    )
    .expect("capability root header");
    let (root_key, root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &TRADING_PROGRAM_ID);
    assert_eq!(
        root_key, chain.root,
        "the Direct root model missed its own address"
    );

    let coordinates = DirectCoordinatesV1::new(chain.market.to_bytes(), core.identity.generation)
        .expect("direct coordinates");
    let mut replays = [0_u64; 2];
    for (index, slot) in replays.iter_mut().enumerate() {
        let maker = chain
            .maker_replays
            .get(index)
            .copied()
            .expect("two maker replays");
        let seeds = MakerReplaySeedsV1::new(
            coordinates,
            direct
                .makers
                .get(index)
                .map(|keypair| keypair.pubkey().to_bytes())
                .expect("two makers"),
        )
        .expect("maker replay seeds");
        let (key, bump) = Pubkey::find_program_address(&seeds.as_slices(), &TRADING_PROGRAM_ID);
        assert_eq!(key, maker, "the maker replay model missed its own address");
        *slot = attempts(bump);
    }

    // The zero-fee fixture enables exactly one Custody route, the seller-terminal
    // one, which is `CUSTODY_ROUTES_V3` slot 0: `gross = 10 * 50 / 100 = 5` and
    // `fee = 5 * 50 / 10_000` floors to ZERO, so `seller_terminal` is the only
    // enabled register. That is also why the transaction makes ONE Custody CPI
    // and not two, which is worth knowing before reading any per-Custody figure
    // taken from this route.
    let route = chain
        .custody_routes
        .first()
        .copied()
        .expect("four declared Custody routes");
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(releases.release_set).expect("release set"),
        chain.market.to_bytes(),
        ExecutionRoleV1::Trading,
        chain
            .maker_replays
            .get(1)
            .copied()
            .expect("buyer root")
            .to_bytes(),
        route.request_digest,
    )
    .expect("caller authority seeds");
    let (caller_key, caller_bump) =
        Pubkey::find_program_address(&caller_seeds.as_slices(), &TRADING_PROGRAM_ID);
    assert_eq!(
        caller_key, route.authority,
        "the Custody caller-authority model missed the address the fixture reports",
    );

    let replay_seeds = CustodyReplaySeedsV1::new(
        chain.market.to_bytes(),
        releases.release_set,
        CallerRoleV1::Trading,
        chain
            .maker_replays
            .get(1)
            .copied()
            .expect("buyer root")
            .to_bytes(),
    );
    let (replay_key, replay_bump) =
        Pubkey::find_program_address(&replay_seeds.as_slices(), &CUSTODY_PROGRAM_ID);
    assert_eq!(
        replay_key, chain.custody_replay,
        "the Custody replay model missed the address the fixture reports",
    );

    let transfer_bump = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(chain.market.to_bytes(), releases.release_set).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .1;

    SearchDepthsV1 {
        market: attempts(market_bump),
        root: attempts(root_bump),
        seller_replay: replays.first().copied().unwrap_or_default(),
        buyer_replay: replays.get(1).copied().unwrap_or_default(),
        custody_caller_authority: attempts(caller_bump),
        custody_replay: attempts(replay_bump),
        custody_transfer_authority: attempts(transfer_bump),
    }
}

#[tokio::test]
async fn the_public_direct_route_holds_its_compute_margin_across_thirty_two_seeds() {
    let artifacts = elves();
    let mut observations = Vec::with_capacity(usize::try_from(GATE_SEEDS).unwrap_or_default());
    let mut refusals: Vec<(u64, String)> = Vec::new();
    let mut depths: Vec<(u64, SearchDepthsV1)> = Vec::new();

    for seed in 0..GATE_SEEDS {
        // Every fixture key is drawn inside here, on this thread, with no
        // environment mutation -- see `with_fixture_seed`.
        let (mut test, direct, instructions, releases) = with_fixture_seed(seed, || {
            let mut test = program_test_without_forced_budget(&artifacts);
            let releases = add_release_waist(&mut test, &artifacts);
            let direct = direct_case(&mut test, releases, &artifacts, false);
            let instructions = direct_top_level_instructions(&direct);
            (test, direct, instructions, releases)
        });

        // If this ever becomes the Registry the gate has quietly turned into a
        // measurement of the continuation, which is a different route with a
        // different margin and is not what was accepted.
        assert_eq!(
            instructions[3].program_id, TRADING_PROGRAM_ID,
            "seed {seed}: the gate must measure the top-level route",
        );

        depths.push((seed, search_depths(&direct, releases)));

        let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
        add_lookup_table(&mut test, &addresses);
        let mut context = start_with_substrate(test, fixture_substrate()).await;

        match submit_v0_observed(
            &mut context,
            &instructions,
            addresses,
            Some(&direct.payer),
            &[],
        )
        .await
        {
            Ok(execution) => observations.push((seed, execution.compute_units_consumed)),
            Err(error) => refusals.push((seed, format!("{error:?}"))),
        }
    }

    // Every seed is reported before anything is asserted. The first version of
    // this loop panicked at the first refusal, which is the one shape of red
    // that tells a reader least: a route over the ceiling refuses on SOME
    // draws, and "seed 13 refused" does not say whether that is one unlucky
    // key or twenty. The sweep costs the same either way, so it finishes.
    for (seed, depth) in &depths {
        println!(
            "SEEDDEPTH\t{seed}\tmarket {} CARRIED x3 (worth {} CU)\troot {}\treplay {} {}\t\
             custody-caller {}\tcustody-replay {}\tcustody-authority {}\tmodelled attempts {}",
            depth.market,
            ATTEMPT_COST_CU.saturating_mul(3 * depth.market.saturating_sub(1)),
            depth.root,
            depth.seller_replay,
            depth.buyer_replay,
            depth.custody_caller_authority,
            depth.custody_replay,
            depth.custody_transfer_authority,
            depth.attempts(),
        );
    }
    for (seed, units) in &observations {
        println!("SEEDCU\t{seed}\t{units}");
    }
    for (seed, error) in &refusals {
        println!("SEEDREFUSED\t{seed}\t{error}");
    }
    assert!(
        refusals.is_empty(),
        "{} of {GATE_SEEDS} seeds REFUSED rather than executed, {} of them executing: {refusals:?}. \
         This is the acceptance criteria for a public trade, and a refusal here is a broken \
         route, not a margin question. If every refusal is ComputationalBudgetExceeded, the \
         route is over the protocol ceiling on those key draws -- see GATE_SEEDS on why a \
         SOURCE change alone redraws every bump depth on this route.",
        refusals.len(),
        observations.len(),
    );

    let (worst_seed, worst) = observations
        .iter()
        .copied()
        .max_by_key(|(_, units)| *units)
        .expect("the sweep ran at least one seed");
    let best = observations
        .iter()
        .map(|(_, units)| *units)
        .min()
        .expect("the sweep ran at least one seed");
    let mean = observations.iter().map(|(_, units)| *units).sum::<u64>() / GATE_SEEDS;

    let band = worst.saturating_sub(best);
    let tolerance = cu_budgets_tolerance(band);
    println!(
        "public Direct route across {GATE_SEEDS} seeds: {best} to {worst} CU, mean {mean}, \
         band {band}, worst margin {} of {PROTOCOL_CEILING}",
        PROTOCOL_CEILING.saturating_sub(worst),
    );
    println!(
        "CU_BUDGETS tolerance for a band of {band} is {tolerance}; a budget for this route \
         would be {} against a ceiling of {PROTOCOL_CEILING} -- {}",
        worst.saturating_add(tolerance),
        if worst.saturating_add(tolerance) > PROTOCOL_CEILING {
            "OVER: by that file's own rule this transaction has stopped fitting, and the \
             band is the reason, not the mean"
        } else {
            "under on THIS draw, which is a fact about these keys and this ELF set, not \
             about the route -- see the floor and the refusal share below"
        },
    );

    // The floor statistic, and the model check that makes it a bound.
    let residuals = observations
        .iter()
        .map(|(seed, units)| {
            let depth = depths
                .iter()
                .find(|(candidate, _)| candidate == seed)
                .map(|(_, depth)| *depth)
                .expect("every executed seed was censused");
            units.saturating_sub(ATTEMPT_COST_CU.saturating_mul(depth.attempts()))
        })
        .collect::<Vec<_>>();
    let floor = residuals.iter().copied().min().expect("one seed");
    let ceiling_of_residual = residuals.iter().copied().max().expect("one seed");

    // Off the 1,500 CU grid means a key moved something that is NOT a bump
    // search, which is the one way the site census below can go stale without
    // anybody noticing. 200 CU of slack because two seeds in the 2026-08-30
    // census carried a real non-search key-dependent term of 140 and 4 CU.
    for (index, residual) in residuals.iter().enumerate() {
        let above = residual.saturating_sub(floor);
        let off_grid = above % ATTEMPT_COST_CU;
        assert!(
            off_grid <= 200 || off_grid >= ATTEMPT_COST_CU - 200,
            "seed at index {index} sits {above} CU above the floor, which is {off_grid} CU off \
             the 1,500 CU grid. Every key-dependent cost on this route is supposed to be bump \
             search depth; something now varies with a participant key that is not a search, \
             and the site census in KEY_VARYING_SEARCH_SITES_V1 no longer explains the band.",
        );
    }

    println!(
        "KEY-INDEPENDENT FLOOR {floor} (residual spread {} over the sweep, one geometric draw \
         wide because six of the {KEY_VARYING_SEARCH_SITES_V1} sites are modelled and the \
         Claims caller authority is not); implied C0 {}; a route whose every search landed \
         first try would cost {}",
        ceiling_of_residual.saturating_sub(floor),
        floor.saturating_sub(ATTEMPT_COST_CU),
        floor
            .saturating_sub(ATTEMPT_COST_CU)
            .saturating_add(ATTEMPT_COST_CU.saturating_mul(KEY_VARYING_SEARCH_SITES_V1)),
    );

    let analytic = |select: fn(&SearchDepthsV1) -> u64, sites: u64| -> u64 {
        sites
            * depths
                .iter()
                .map(|(_, depth)| select(depth))
                .max()
                .unwrap_or_default()
    };
    let swept_worst =
        floor
            .saturating_sub(ATTEMPT_COST_CU)
            .saturating_add(ATTEMPT_COST_CU.saturating_mul(
                analytic(|d| d.root, 1)
                    + analytic(|d| d.seller_replay, 1)
                    + analytic(|d| d.buyer_replay, 1)
                    + analytic(|d| d.custody_caller_authority, 1)
                    + analytic(|d| d.custody_replay, 1)
                    + analytic(|d| d.custody_transfer_authority, 1)
                    + ceiling_of_residual.saturating_sub(floor) / ATTEMPT_COST_CU
                    + 1,
            ));
    println!(
        "ANALYTIC WORST over the deepest draw each of the {KEY_VARYING_SEARCH_SITES_V1} sites \
         made in THIS sweep: {swept_worst} against a ceiling of {PROTOCOL_CEILING}. This is not \
         a bound on a stranger's keys either -- it is a per-site maximum over 32 draws, and it \
         moved 1,441,743 -> 1,399,742 between two commits that changed the route's cost by \
         1 CU. The Market is absent from this sum because it no longer searches; the deepest \
         Market draw in this sweep would have added {} CU to it.",
        ATTEMPT_COST_CU.saturating_mul(
            3 * depths
                .iter()
                .map(|(_, depth)| depth.market)
                .max()
                .unwrap_or_default()
                .saturating_sub(1)
        ),
    );

    assert!(
        floor <= TOP_LEVEL_KEY_INDEPENDENT_CU_V1,
        "the public Direct route's KEY-INDEPENDENT cost is now {floor} CU, past the \
         {TOP_LEVEL_KEY_INDEPENDENT_CU_V1} gate. This number does not move when the keys or the \
         bump depths move, so this red is a CODE change and nothing else -- worst seed \
         {worst_seed} at {worst} CU is not the evidence and must not be used to argue the point. \
         Find the change before raising this number: only {} CU stand between a first-try route \
         and the protocol ceiling. Check the shared contract crates first -- the last change to \
         cost this route real margin believed it had changed no program at all.",
        PROTOCOL_CEILING.saturating_sub(
            floor
                .saturating_sub(ATTEMPT_COST_CU)
                .saturating_add(ATTEMPT_COST_CU.saturating_mul(KEY_VARYING_SEARCH_SITES_V1))
        ),
    );
}

// ---------------------------------------------------------------------------
// The control that makes the seven-site census believable.
//
// The sweep above asserts that the fixture STAGES the founding's bumps. That is
// necessary and it is not sufficient: a reader could carry a bump and never
// check it, and a fixture staging the canonical value would look identical
// either way. What separates "the carry is live" from "the carry is decorative"
// is a WRONG bump, so this control stages one and requires the refusal.
//
// It is also the reason a bump is safe to carry at all. `StateBumpsV1`'s own
// doc is the claim -- "a wrong bump reproduces a different address, so a reader
// that compares refuses" -- and this is the executable form of it, on the real
// ELFs, at the top level, once per carried site.
// ---------------------------------------------------------------------------

/// Which recorded bump the control bends before submitting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TamperedBumpV1 {
    /// The founding's own bumps, untouched. This arm must EXECUTE.
    None,
    /// Read by Trading `hot_v3`, Claims `sparse_native_transfer_v1`, Custody `lib`.
    Market,
    /// Read by Custody `authenticate_realm`, raw record half.
    RealmRaw,
    /// Read by Custody `authenticate_realm`, staging cursor half.
    RealmStaging,
}

/// One nonzero bump that is not `canonical`.
///
/// Not zero, because zero is `StateBumpsV1`'s UNRECORDED encoding and would
/// degrade to a search -- which is the behaviour this control exists to
/// distinguish itself from, not a wrong bump at all. One step down is enough:
/// `create_program_address` either refuses the seeds outright or lands on some
/// other address, and the reader compares.
const fn bend(canonical: u8) -> u8 {
    if canonical <= 1 {
        canonical + 1
    } else {
        canonical - 1
    }
}

/// The custom program code a refusal carried, so this file can name it.
fn refusal_code(refusal: &RefusedExecution) -> Option<u32> {
    match &refusal.error {
        BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(code),
        ))
        | BanksClientError::SimulationError {
            err: TransactionError::InstructionError(_, InstructionError::Custom(code)),
            ..
        } => Some(*code),
        _ => None,
    }
}

/// Submit seed `seed`'s canonical Direct trade with one bump bent, or none.
async fn execute_with_bump(
    seed: u64,
    tamper: TamperedBumpV1,
) -> Result<u64, (Option<u32>, String)> {
    let artifacts = elves();
    let (mut test, direct, instructions, _releases) = with_fixture_seed(seed, || {
        let mut test = program_test_without_forced_budget(&artifacts);
        let releases = add_release_waist(&mut test, &artifacts);
        let direct = direct_case(&mut test, releases, &artifacts, false);
        let instructions = direct_top_level_instructions(&direct);
        (test, direct, instructions, releases)
    });
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    if tamper != TamperedBumpV1::None {
        let mut account = context
            .banks_client
            .get_account(direct.chain.market)
            .await
            .expect("market account read")
            .expect("the fixture installed a live Market state");
        let mut state = CoreState::decode(&account.data).expect("the planted Market state decodes");
        let recorded = |bump: Option<u8>| -> Option<u8> {
            Some(bend(bump.expect(
                "the control requires a staged bump to bend; a `None` here means the fixture \
                 went back to StateBumpsV1::UNRECORDED and there is nothing to prove",
            )))
        };
        match tamper {
            TamperedBumpV1::None => {}
            TamperedBumpV1::Market => state.bumps.market = recorded(state.bumps.market),
            TamperedBumpV1::RealmRaw => {
                state.bumps.realm_raw_record = recorded(state.bumps.realm_raw_record);
            }
            TamperedBumpV1::RealmStaging => {
                state.bumps.realm_staging_record = recorded(state.bumps.realm_staging_record);
            }
        }
        account.data = state
            .encode()
            .expect("a bent bump is still a representable CoreState")
            .to_vec();
        context.set_account(&direct.chain.market, &AccountSharedData::from(account));
    }

    submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .map(|execution| execution.compute_units_consumed)
    .map_err(|refusal| (refusal_code(&refusal), format!("{:?}", refusal.error)))
}

#[tokio::test]
async fn a_wrong_recorded_bump_refuses_instead_of_reaching_the_account_it_names() {
    let executed = execute_with_bump(0, TamperedBumpV1::None).await.expect(
        "the founding's own bumps must execute; without this arm the three below prove \
                 only that the fixture can be broken",
    );
    println!("BUMPCONTROL\tuntouched\tEXECUTED\t{executed} CU");

    let (market_code, market_error) = execute_with_bump(0, TamperedBumpV1::Market)
        .await
        .expect_err(
            "a WRONG Market bump executed. Then no reader compares the address it reproduced \
             against the account it was handed, the carry is not a derivation but a hint, and \
             `StateBumpsV1`'s safety argument is false.",
        );
    println!("BUMPCONTROL\tmarket\tREFUSED\t{market_code:?}\t{market_error}");
    assert_eq!(
        market_code,
        Some(TradingSbfError::Content as u32),
        "the wrong Market bump refused as {market_error} rather than in Trading's own reader. \
         Trading is the top-level program and `market_core_state_address_v2` runs before any \
         CPI, so a different code means the refusal came from somewhere else and this control \
         is not measuring what it claims.",
    );

    let (raw_code, raw_error) = execute_with_bump(0, TamperedBumpV1::RealmRaw)
        .await
        .expect_err(
            "a WRONG realm raw-record bump executed, so Custody's realm reader compares nothing",
        );
    println!("BUMPCONTROL\trealm-raw\tREFUSED\t{raw_code:?}\t{raw_error}");
    let (staging_code, staging_error) = execute_with_bump(0, TamperedBumpV1::RealmStaging)
        .await
        .expect_err(
            "a WRONG realm staging-cursor bump executed, so Custody's realm reader compares \
             nothing",
        );
    println!("BUMPCONTROL\trealm-staging\tREFUSED\t{staging_code:?}\t{staging_error}");

    // Both realm halves are read by `require_realm_authority` inside Custody and
    // refuse as its one realm code, which is NOT Trading's -- so the three arms
    // partition exactly as the reader map in KEY_VARYING_SEARCH_SITES_V1 says
    // they should, and no arm is refusing for a shared unrelated reason.
    //
    // NOT VERIFIED HERE, and said rather than papered over: Custody's variant is
    // not NAMED. `CustodySbfError` lives in `dclutch-custody-sbf`, which is not
    // a dependency of this crate, and AGENTS.md forbids writing the discriminant
    // as a bare number. Adding the dependency is a Cargo.toml change; the lane
    // that makes it should turn these two assertions into named ones. Observed
    // on 2026-08-30 the two arms both refuse as 24,580 -- which IS Custody's
    // realm code, from its own reader -- and Trading's arm as 16,387, which is
    // `TradingSbfError::Content` and is named above.
    assert!(
        raw_code.is_some() && raw_code == staging_code,
        "the two realm-record halves refused differently ({raw_code:?} vs {staging_code:?}), \
         which the single `require_realm_authority` comparison cannot do: {raw_error} / \
         {staging_error}",
    );
    assert_ne!(
        raw_code,
        Some(TradingSbfError::Content as u32),
        "the realm bumps refused in Trading, which does not read them. Either the tamper is \
         being caught before Custody runs, or this control is bending the wrong field.",
    );
}
