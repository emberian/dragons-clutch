# Decision 0015: the two dead devnet markets are untradeable, not unredeemable — so what do we do with them?

Status: **OPEN — ember's ruling required.** Ledger M-22. The premise of the
question does not survive contact with the tree; §2 and §3 say why, and the
decision that remains is smaller, cheaper, and genuinely a values call.

## 1. The question, as it should now be asked

M-22 asks what happens to *"a first market that can never be redeemed."* For
the two devnet markets it is usually applied to, the answer is: **they can be
redeemed.** They can be resolved, redeemed and retired through the ordinary
lifecycle, today, with no new protocol machinery.

So the real question is a choice, not a gap: **do we walk them through
resolution and retirement — recovering their rent and paying their claims — or
keep them open as the honest public record of the wall the project hit?**

## 2. Three corrections to the premise

**There are two such markets on devnet, not four.** `market17` is not a market;
it is the sealed founding-input artifact for `7Mcu1ZT9…`
(`docs/evidence/TRADE_DIRECT_ACTIVATION_WALL_2026_08_29.md:40`). `market18` is
`9JwhTHyx…`, and it is the market that **can** trade — it holds the first
capability root any dClutch market has had (`GOAL.md:139-145`). The dead set is
`7Mcu1ZT9…` and `CasyDFow…`.

**`custody_context` is not why, and the market it *is* about is not on devnet.**
That defect is real and is decision 0008 §6.4's subject, but it bit a
*local-validator* market, `4fQNy8k7…`, whose own fixture says so: *"Finalized
account bytes copied verbatim off a local successor-campaign validator. Not
devnet or mainnet evidence."*
(`apps/dclutch-web/fixtures/live-open-market.json:3`). It was fixed
2026-08-27; the devnet pair was founded 08-29 on cohort-6 with the fix in, and
the `7Mcu1ZT9…` census reconciles its Hoard against its aggregate with every
conservation law holding — which is only possible if `custody_context`
addresses the real vault
(`docs/evidence/TRADE_FLAGSHIP_FIRST_AUDIT_2026_08_30.md:14-27`).

**So M-22 names one condition and the archaeology added two markets with a
different one.** `4fQNy8k7…` is genuinely unredeemable — a wrong vault address
sealed into an immutable aggregate — and it is a local fixture, not a public
object. `7Mcu1ZT9…` and `CasyDFow…` are **untradeable**: the fault is in their
capability manifest, and the redemption path does not go through it.

## 3. Why they are untradeable, and why that does not block redemption

**Untradeable, permanently, and it is sealed into the market's own address.**
The `CapabilityProgramSetV2` has three entries and none passes both of Trading's
gates: `InlineOrdinary` refuses on schema
(`programs/dclutch-trading-sbf/src/outer.rs:948`), and the two V1 entries
declare `root_state_bytes = 24` while their sealed effects project
`request_bytes = 0`, refusing at `outer.rs:1442`. The manifest digest is one of
the Market PDA's nine identity seeds
(`crates/dclutch-market-core-codec/src/physical.rs:620-662`, `:648`), enforced
at founding (`programs/dclutch-core-sbf/src/found.rs:406-412`) — *"No record
republication can add a fourth entry to THIS market."* The activation deadline
has since elapsed, enforced twice
(`programs/dclutch-core-sbf/src/capability.rs:526`;
`crates/dclutch-capability-contract/src/funding.rs:1188-1191`).

**Resolution is a different authority, and it is intact.** `admit_terminal`
requires `require_admission(*state, admission, Role::Resolution)`
(`crates/dclutch-market-core-codec/src/generated.rs:1005`), and
`require_admission` checks the market's **release set** against the Registry
activation cache — `admission.market_release_set_id !=
state.identity.selected_release_set`, `admission.selected.release_set_id`,
`admission_valid(admission, role)` (`generated.rs:1216-1225`). **It never reads
the capability manifest.** The per-market capability wall that kills trading has
no bearing on the five-role execution admission that resolution runs under. The
wall evidence itself already said so in one word: *"`7Mcu1ZT9` remains Open,
**admittable**, and permanently untradeable"*
(`TRADE_DIRECT_ACTIVATION_WALL_2026_08_29.md:66-68`).

**And the market cannot fail to have an answer.** Its outcome set includes a
source-failure outcome — *"A window the feed cannot answer resolves to the
source-failure outcome instead — silence is an outcome here, not a stall"*
(`apps/dclutch-web/fixtures/market-registry.devnet.json:19-27`). So a terminal
receipt exists for every future, including the one where Pyth never answers.

**The lifecycle therefore closes:** `Open` →(`admit_terminal`, Resolution
role)→ `Terminal` →(`begin_retiring`, permissionless since the C0 weld)→
`Retiring` →(`retire`)→ `Retired`, returning `core_account_lamports` to the
RentCredit (`generated.rs:1177-1201`). Redemption is open the whole way: both
payout routes gate on `CorePhaseGateV3::TerminalOrRetiring`
(`programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs:412`;
`rational_terminal_v3.rs:266`).

**And the claims pay out at par whichever outcome wins.** The founder Position
holds 500,000,000 claims at *each* of four outcomes
(`TRADE_FLAGSHIP_FIRST_AUDIT_2026_08_30.md:20-23`) — a complete set, against a
fully collateralized Hoard. A complete set is the one holding whose value does
not depend on the winner. Retirement's own precondition, an economically empty
market (`generated.rs:1183-1189`), is reachable precisely because redeeming that
set drains the Hoard exactly.

## 4. What is genuinely absent

Stated plainly, so the ruling is not mistaken for "everything is fine":

- **There is no abandoned/void phase.** Five variants, `Founding, Open,
  Terminal, Retiring, Retired` (`generated.rs:336-343`); `Abandoned`, `Void`,
  `Escheat`, `Defunct`, `Dormant` return zero market-lifecycle hits. A market
  that is permanently untradeable is indistinguishable, on chain, from one whose
  trading has not started.
- **There is no successor pointer.** No `superseded_by` field in
  `MarketIdentity` or `CoreState` (`generated.rs:321-333`, `:353-365`);
  `generation` is a *seed* (`physical.rs:652`), so re-founding makes a different
  PDA with no link back. Q1A's lineage design re-points a market's *release
  set*, names the capability manifest as the orthogonal degree of freedom
  (`docs/design/RELEASE_LINEAGE_MIGRATION_V1.md:231`), and forbids changing role
  program ids (`:35`) — so it neither rescues nor links these.
- **A market that can reach neither trading nor resolution would strand
  everything.** If a market's *resolution* authority were also unreachable, the
  chain `Open → Terminal → Retiring → Retire` would break at the first arrow and
  every lamport would be stranded permanently — the lamport statement already
  names two flow classes as terminal, *"no route can ever return them"*
  (`tools/lamport-ledger/README.md:162-167`), against a measured 121 accounts
  per founded market with 78% of the rent bill in registry records
  (`GOAL.md:406-415`). **No market is in that condition today.** The class is
  currently empty, and that is worth writing down as a fact rather than assuming
  it is a guarantee.

**Q3C's compaction does not bear on any of this**, and the answer to "does it
change when compaction lands?" is no. Compaction runs in `Terminal` and
`Retiring` only (`docs/design/CLAIM_CHECK_COMPACTION_V1.md:332-340`) and cannot
run in `Open`, because the crank calls redemption's own payout derivation while
`CoreState::valid_static` makes `Open` structurally require
`terminal_receipt.is_none()` (`generated.rs:375-379`; refusal `0x5604`). It is
the disposition route for markets that *resolved* and have sleeping holders — a
different problem. It has also not shipped: the two modules hold refusal enums
only, no `process`, no dispatcher arm
(`programs/dclutch-claims-sbf/src/claim_check_compaction_v1.rs`).

## 5. Options

| option | cost | what it does |
|---|---|---|
| **A. Leave them Open, as witnesses** | zero | The site already tells both stories honestly — *"its trading window was never switched on… readable forever, exactly as founding left them"* (`apps/dclutch-web/fixtures/market-registry.devnet.json:19-30`). Rent stays spent. The founder's collateral stays locked in the Hoard, recoverable at any later date. |
| **B. Resolve, redeem, retire** | no new protocol code; one keeper run per market | Walks the existing lifecycle. Recovers the rent, returns the collateral, ends with `Phase::Retired`. Destroys the accounts the site currently reads live. |
| **C. A, plus an honest product bucket** | web only, hours | Today all three devnet markets render under *"The markets that are open"* because all three are `Phase::Open`; the buckets that exist are `settled`, `founding`, `unreadable` (`apps/dclutch-web/components/MarketDiscoveryWorkspace.tsx:355-361`), and **a permanently-untradeable open market has no honest one to sit in.** The editorial registry already holds the words. |
| **D. Build an abandoned phase or an unwind route** | large | A sixth `Phase` touches the 360-byte `CoreState`, its Lean-generated codec, `valid_static`, every phase gate across five programs, and the refusal census. |

## 6. Recommendation

**Rule C now. Hold A for the two markets. Refuse D. Keep B available and
un-run.**

**C, now, because it is the only real defect on the list.** A market that can
never trade is currently filed under "open", which is the one thing on the site
that is not true. The registry already carries the honest sentence for each;
the work is a bucket and a card state. This is also the option the project's
own ranking function selects — a gap that stops a stranger understanding
outranks a gap in protocol completeness (`docs/INTENT.md` §1).

**A over B, for these two.** B is available and that is the important fact;
running it is a separate question and the answer is no, for now. Their value as
witnesses is higher than their rent. They are the protocol's own public record
of a wall it hit and reported honestly, they are the first two markets ever
founded on this chain, and the site reads them live. Retiring them would
convert a legible artifact into a closed account to recover a bounded amount of
already-spent devnet rent — the wrong trade, and one that cannot be undone.
**Ember may reasonably rule the other way**, and if the answer is B, the
argument is hygiene: leaving fully collateralized principal parked in a market
nobody will ever trade is a standing untidiness, and demonstrating the full
lifecycle end-to-end on a real public market has its own demo value. That is a
genuine values call and it is the one this record exists to put.

**Refuse D.** A sixth phase is the most expensive way to say something the
market's own accounts already prove. "Permanently untradeable" is a fact
*about* a market — derivable from its elapsed deadline and its sealed manifest
— not a state it should transition into. The phase enum is matched on in five
programs and its narrowness is load-bearing.

**One thing to record rather than build.** The predicate "this market can never
activate" is unusually **decidable**: the activation deadline slot is strictly
below the current slot (`capability.rs:526` already computes exactly that
comparison), and the capability manifest is sealed into the Market PDA's seeds,
so the admissible entry set is fixed at founding and enumerable from the
market's own address. If the class ever stops being a two-instance
curiosity — or if a market ever lands whose *resolution* is also unreachable,
the case §4 shows would strand everything — that predicate is what a disposition
route would be built on, and the protocol already has the shape twice for
pre-completion artifacts
(`programs/dclutch-registry-sbf/src/record_v1.rs:510, 528`, *"an abandoned
record set can always be reclaimed and never strands its rent"*;
`programs/dclutch-core-sbf/src/series_permit_expiry.rs:1-7`). What does not
exist is any analogue after founding completes. Writing that sentence down is
the whole of the protocol-side work this record recommends.

## 7. What changes downstream once ruled

- **C ruled** → one web lane: a discovery bucket, a card state, and a headline
  market that is not one of the two dead ones (`public-cut.devnet.json` pins
  `7Mcu1ZT9…` with `trade`/`resolve`/`redeem` all `null`).
- **A ruled** → M-22 closes with a decision instead of an inventory row, and
  decision 0008 §6.4's two options — re-found, or keep as witness — gain a
  correction: its subject is the local market, and the devnet pair it has been
  read onto were never in its condition.
- **If B is ruled instead** → a keeper run per market, no code, and the site
  copy changes from "readable forever" to a settled history.
- Either way, one line belongs in `docs/OMISSION_INDEX.md`: **a market whose
  resolution authority is unreachable would strand every lamport it holds, and
  nothing in the protocol prevents founding one.** That is the honest residual,
  and it is not what M-22 asked about.
