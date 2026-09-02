# Decision 0015: the two dead devnet markets are untradeable, not unredeemable — so what do we do with them?

Status: **RULED B, THEN FOUND UNEXECUTABLE.** Ledger M-22. Ember ruled option B
— resolve, redeem, retire — on 2026-08-30 (`WAVE.md`, evening rulings, E2).
Executing it that evening established that **B cannot be run at all on the
deployed cohort**, for a reason the options table below priced at zero and §3
never looked for: the claims can only be burned by a signer whose key nobody
holds. **§8 is the execution record and supersedes option B's cost line.** The
rest of the document stands as written and is confirmed on chain.

**FINAL DISPOSITION (ember, 2026-08-30 evening, `458d47bb`).** The founder keys
are gone from all wallets and **the write-off stands** — *"It's just devnet sol
it isn't a big deal."* Consequences, each now decided rather than pending:

- The two markets **remain standing as unretireable**. Option A is what the
  chain will show, not by choice between A and B but because B is closed.
- **No pre-cut retirement contingency.** §8.6's ordering constraint — that B
  must run before the cohort-7 cut, on a cohort-revision client, if the key
  ever surfaces — is not being held open; the cut proceeds.
- **Option C is RULED, and EXECUTED the same evening** (BUCKET, `e3600765`) —
  the reader-burden concern is *"solved editorially … not by deletion."*
  §8.8's *"there is no honest registry sentence to add until ember rules on
  C"* is discharged. The separation is made from the two facts that decide it
  and never by restating the phase: `marketActivationOutlookV1` reads the
  verdict off the card's own authenticated manifest — every entry
  deadline-activated and strictly below the card's finalized slot, with no
  capability outstanding — so every card still prints `Open`, and a manifest
  that does not authenticate stays UNKNOWN rather than becoming a claim that a
  market is finished. §5's editorial registry is untouched, as §8.8 required.
  **What C did not reach**: the generated reference and the public docs
  landing still say *"there is no open market"*
  (`tools/genref/generate.mjs:406`, `tools/genref/render-site.mjs:453,470`,
  `docs/reference/README.md:35`, `README.md:14`) — the same defect one layer
  up, listed in
  `docs/evidence/SLIPPED_THROUGH_SWEEP_2026_08_30.md` §3.

§8.7's write-off is therefore final: **167,999,880 lamports of rent and
1,000,000,000 collateral atoms**, plus market18's further 94,022,640 lamports
and 500,000,000 atoms under the same lost owner.

The premise of the original question does not survive contact with the tree;
§2 and §3 say why, and the decision that remained looked smaller, cheaper, and
genuinely a values call. It was smaller than M-22 asked. It was not cheap.

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
payout routes gate on `CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1`
(`programs/dclutch-claims-sbf/src/market_admission_v1.rs`, read by
`terminal_settlement_v3.rs` and `rational_terminal_v3.rs`; it was
`CorePhaseGateV3::TerminalOrRetiring` until the guards took a name the route
census can read).

**And the gate that would otherwise block retirement is clear for exactly the
reason these markets are stuck.** Both `retire` (`generated.rs:1179-1181`) and
Claims' market closure
(`programs/dclutch-claims-sbf/src/market_closure_v1.rs:591`) refuse unless
`outstanding_capabilities == 0`. That counter is incremented in **one place
only** — `activate_capability`, `generated.rs:1081-1084`, itself gated on
`Phase::Open` — and decremented by `close_capability_child` (`:1109-1112`). The
markets never activated a capability, so the counter has stood at its founding
zero (`:832`) since the day they were founded. **The failure that makes them
untradeable is the same fact that leaves their retirement path unobstructed.**

**And the claims pay out at par whichever outcome wins.** The founder Position
holds 500,000,000 claims at *each* of four outcomes
(`TRADE_FLAGSHIP_FIRST_AUDIT_2026_08_30.md:20-23`) — a complete set, against a
fully collateralized Hoard. A complete set is the one holding whose value does
not depend on the winner. Retirement's own precondition, an economically empty
market (`generated.rs:1183-1189`), is reachable precisely because redeeming that
set drains the Hoard exactly.

**Resolution is the *only* way out, because the in-`Open` merge is dead code.**
It is worth stating what does *not* work, since a reader will reach for it
first. A complete set looks like it should be mergeable back to collateral
without knowing the winner, and the economics kernel agrees — `admit_basket_phase`
explicitly admits `MergeCompleteSet` in `Phase::Open`
(`crates/dclutch-economic-slice-kernel/src/lib.rs:675-690`). It is unreachable
anyway, for three reasons that have nothing to do with these markets being
stuck:

- **It addresses accounts that do not exist.** The generic `DCLTCPK1` route
  derives the aggregate under `b"dclutch:claims-aggregate:v1"`
  (`crates/dclutch-claims-svm/src/lib.rs:49`), while a market founded through
  the live path holds LiabilityBasisV2 state under `b"dclutch:lbv2:market"`
  (`crates/dclutch-claims-svm/src/founding_v5.rs:27`). The identity check
  refuses first — `claims_aggregate.key != &expected_aggregate` →
  `ClaimsSbfError::Identity` (`programs/dclutch-claims-sbf/src/lib.rs:1196-1203`).
- **Nothing on chain emits it.** The route's authority must be a
  `CallerAuthoritySeedsV1` PDA under an activated role program
  (`lib.rs:1000-1006`), so no wallet can sign it — and the dispatcher's own
  comment names three consumers of which one builds only the *mint* direction,
  one does not exist, and one emits other actions
  (`lib.rs:364-372`).
- **It would move no collateral if it ran.** Its 13-account frame carries no
  Custody program and no vault; the "payout" is written into a receipt field,
  not transferred.

So the single Hoard→External path in Claims is `execute_terminal_custody_v3`
(`rational_terminal_v3.rs:332-349`), reachable only from the two
`TerminalOrRetiring` routes; and there is no Open-phase merge-out at all. The
planner this line used to cite — `trading-sbf/src/direct/complementary.rs` —
was reached by no route and has been deleted; `MergeRegistered` (6) and
`MergeInline` (14) refuse at
`programs/dclutch-trading-sbf/src/hot_v3.rs:5986-5988` with
`UnsupportedContent`, because no crosscheck planner for them exists. **Terminal
admission is not merely the cheapest route out — it is the only one**, and it
is now the only one by a wider margin than this decision measured.

A small honesty note that follows from this: the web portfolio reports
`kind: 'mergeable'` for any Open market with a complete set
(`apps/dclutch-web/lib/portfolio.ts:150-155`). Its own text is careful —
*"This is arithmetic on these balances, not an offer"* — and it is accurate as
arithmetic, but on these two markets it names an operation the protocol cannot
perform. Worth a word when option C's bucket is built.

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
lifecycle end-to-end on a real public market has its own demo value. **And
there is no middle setting** — §3 shows the only collateral egress is the full
resolve-then-redeem sequence, so "recover the principal but keep the market
readable" is not on the menu. That is a genuine values call and it is the one
this record exists to put.

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

## 8. Execution record, 2026-08-30 (TRADE-3): option B is not executable

Ember ruled B — *"Delete that shut and get rid of it; it burdens the reader
with a detail that only matters to us"* — and routed execution to the devnet
lane. Nothing was written to devnet. Every fact below is a finalized read at
slot 490640345 or a citation in the deployed cohort's own source.

### 8.1 What §3 got right, confirmed on chain

The retirement gate really is clear, and for exactly the reason §3 gave.
Decoding both markets' 360-byte `CoreState` (Core `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N`):

| | `7Mcu1ZT9…` | `CasyDFow…` | `9JwhTHyx…` (market18, control) |
|---|---|---|---|
| phase / readiness | Open / Consumed | Open / Consumed | Open / Consumed |
| `outstanding_capabilities` | **0** | **0** | **1** |
| `terminal_receipt` | none | none | none |
| `selected_release_set` | `FP4nnEfV…` | `FP4nnEfV…` | `FP4nnEfV…` |
| generation | 2 | 2 | 2 |

The counter has stood at its founding zero exactly as §3 predicted, and
market18 — the one market that *did* activate a capability — reads 1, which is
the control that proves the field is live rather than always zero.

### 8.2 The step nobody priced: the aggregate cannot be emptied

`retire` refuses unless `claims.aggregate_empty`
(`crates/dclutch-market-core-codec/src/generated.rs`, the `!claims.aggregate_empty`
conjunct), and Core does not observe that itself — `retire_v1.rs`'s
`plan_retired_transition` passes the flag as a literal `true` and the real
proof is the Claims CPI. `authenticate_empty_aggregate`
(`programs/dclutch-claims-sbf/src/market_closure_v1.rs`) walks **every**
outcome's supply and returns `ClaimsMarketClosureSbfErrorV1::Liability` unless
all of them are zero. So retirement requires burning every claim first, which
is what §5's option B called "redeem".

Enumerating the devnet Claims program's own accounts, every claim of both dead
markets sits in a single founder Position:

| market | aggregate | founder Position | holding |
|---|---|---|---|
| `7Mcu1ZT9…` | `669xTVjB…` | `FqT63Tkx…` | 500,000,000 at each of 4 outcomes |
| `CasyDFow…` | `35gGqHcC…` | `nvxjVpJy…` | 500,000,000 at each of 4 outcomes |
| `9JwhTHyx…` | `2SiorXJp…` | `4YSPmPTz…` | 500,000,000 at each of 4 outcomes |

Every other Position on the cluster — the four participant admissions — holds
`[0, 0, 0, 0]`. **All three founder Positions name the same owner:**
`6LkyGdwJcCWaGRZPc9DKYqtabvABgXuyPLTHeGJvRdoS`.

### 8.3 Only that owner can burn them, and the route says so in its own words

`programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs` binds the payout
signer, and the comment above the check is the whole argument:

> Coordinate 0 is a signer in every mode; WHOSE signature is what the role
> selects. Under `Claims` there is no caller program to derive an authority PDA
> under, and the entitled party is the Position's owner. … The one exception is
> the permissionless compaction crank, and it is an exception to WHO signs
> rather than to whether anyone does.

Three consequences, in the order a reader will reach for them:

- **The Claims role needs the owner's key.** `caller_role == Claims` demands
  `accounts[0].key == input.owner`.
- **A role-program caller would skip that check — and is unreachable here.**
  The non-`Claims` roles authenticate a caller-authority PDA under an
  *activated* role program. For Direct that is the capability root §3 proves
  these two markets can never have: the activation deadline (`490330281`)
  elapsed over 310,000 slots before this reading, and the manifest is sealed
  into the Market PDA's seeds. **The one fact that makes them untradeable is
  the same fact that removes the only signature-free way to empty them.** §3's
  closing line — "the failure that makes them untradeable is the same fact that
  leaves their retirement path unobstructed" — is half true: it unobstructs the
  *phase* transition and obstructs the *burn* that has to precede it.
- **Compaction does not rescue it.** Claim-check compaction shipped on
  2026-08-30 but is not on the deployed cohort — it rides cohort-7 — and its
  crank is entitled by an *elapsed* deadline (180 days), not by need.

**The quotation above is from `main`; the deployed program is stricter.** The
owner binding itself landed in `082f942f` (2026-08-27), before the cohort-6
deployment and before these markets were founded, so the ELF that actually owns
them carries the check — and carries it *without* the compaction exception,
which is newer than the deployment. The conclusion therefore holds a fortiori
on chain. **What was not done:** no transaction was built or simulated against
the deployed programs, because §8.6's codec width refuses these markets in every
client built from `main`. This finding rests on reading the deployed cohort's
source together with finalized chain state, not on an observed refusal.

### 8.4 The key is gone

The board records it (TRADE, 2026-08-29 20:25): *"seller = founding founder
6LkyGdwJ … key at `scratchpad/founder-ids/founding-founder.json`"*. That
directory no longer exists in the orchestrator scratchpad, and TRADE's own
transcript was lost to an account rotation. What was searched, and found:

- **Index-0 sweep** — every 64-byte keypair JSON under `/private/tmp`,
  `/Users/ember` and `~/.config/solana`, matched by the public key stored in
  its own bytes 32..64. The only surviving identity of the 2026-08-29 founding
  is the **campaign payer** `GZQoAjVB…`. Not found: the founder, the
  substituted founder, **or either live market's collateral mint** — and those
  mints had to sign their own creation, so what is missing is the founding key
  *directory*, not one file out of it.
- **Derivation sweep** — `seed.rs` makes a persisted role's index *n* > 0
  reproducible as `SHA-256(DOMAIN ‖ 0 ‖ file-secret ‖ 0 ‖ role ‖ 0 ‖ n)`, so a
  key can outlive its file if a *sibling* file survives. 445,440 derivations
  (both domains × all 10 role names × indices 0..255 × 88 candidate keypair
  files, ed25519 derivation self-checked against a known pair) reproduced
  neither the founder nor either collateral mint.
- hbox holds only the loopback campaign's deterministic founder
  (`2SVqjPNY…`, a `--keypair-seed` key, refused on any non-loopback endpoint).

**Not verified, and the one avenue left:** ember's own machines, wallets and
backups were not searched. If `6LkyGdwJ`'s secret exists anywhere, option B
becomes executable again and §8.6's ordering applies.

### 8.5 Why nothing forced anyone to hold it

This is a design gap, not an operator slip. The founder **never signs at
founding** — it is only the identity the complete set is minted to — so:

- `campaign --founding-only` takes `--founding-founder PUBKEY`, and
  `role::FOUNDING_FOUNDER` is deliberately excluded from both
  `FOUNDING_REQUIRED_ROLES` and `KEYPAIR_ROLES`
  (`tools/local-validator/bootstrap/successor/src/campaign.rs`, asserted by its
  own tests);
- `tools/release/stage-devnet-sponsored-market-open.sh` asked for it as *"two
  distinct public identities"* with no custody obligation at all.

The founder's key is worthless for one transaction and load-bearing for the
market's entire remaining life. Nothing in the tooling said so.
**Fixed the same evening** (TRADE-3, `tools/release`): the emitted execute
wrapper now demands `DCLUTCH_FOUNDING_FOUNDER_KEYPAIR`, derives the identity
from the file, refuses a disagreeing `DCLUTCH_FOUNDING_FOUNDER`, and refuses a
substituted founder equal to it — proven red against the prior revision by
`tools/release/test-stage-devnet-sponsored-market-open.sh`.

### 8.6 A second, separately sufficient blocker

Even with the key, the client would refuse before the chain did.
`e93fe5e9` widened `CoreState` from 360 to 368 bytes and `decode` refuses
`input.len() != STATE_BYTES`, so **every operator builder and driver built from
`main` today refuses all three devnet markets at planning**, and after the
cohort-7 cut the deployed programs will refuse them too. This is CORESTATE's
"cohort isolation is FALSE for reading" landing on the retirement path. Any
future attempt at B must be driven by a client built at the cohort's own
revision, and must run **before** the cut, not after.

### 8.7 What is stranded, exactly

Directly attributable across the seven protocol programs, at finalized reads:

| | `7Mcu1ZT9…` | `CasyDFow…` |
|---|---|---|
| protocol accounts | 15 | 13 |
| rent held | **88,454,640 lamports** (0.088455 SOL) | **79,545,840 lamports** (0.079546 SOL) |
| of which its RentCredit | `5uG8Qfeu…` 18,075,120 | `7b1cNBrK…` 18,075,120 |
| Core state account | 3,396,480 | 3,396,480 |
| collateral mint | `6odqARs4…` | `7rswmACU…` |
| Hoard vault | `6aDbBXDY…` | `GvynyL3w…` |
| **collateral locked** | **500,000,000 atoms** | **500,000,000 atoms** |

Total rent stranded by this finding: **167,999,880 lamports ≈ 0.168 SOL**,
plus 1,000,000,000 collateral atoms across two devnet mints. The wallet is
ember's devnet development wallet
`4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`; the rent was already spent at
founding, so this is a **write-off, not a new debit** — the loss is that it can
never be recovered. Not counted here: registry records shared across markets or
addressed by content digest rather than by market address, which the
attribution sweep leaves in a 1.846-SOL unattributed pool.

**Market18 has the same owner and therefore the same exposure**: a further
94,022,640 lamports and 500,000,000 atoms, plus the live capability root that
makes it the market the site points at.

### 8.8 What changes

- **B is closed as unexecutable**, not as refused. Its cost line in §5 —
  "no new protocol code; one keeper run per market" — was wrong by a step that
  cannot be run at all, and §7's "if B is ruled instead → a keeper run per
  market, no code" is withdrawn.
- **Option C is un-mooted.** E2 moots C only if B runs. It did not, so the two
  markets remain `Phase::Open` and the site still files a permanently
  untradeable market under "the markets that are open" — the one untrue thing
  §5 named. C is a web lane's work and it is now the live disposition.
- **The editorial registry was deliberately left unchanged.** Its stories say
  these markets stay *"readable forever, exactly as founding left them"*, which
  is now more true than when it was written. Editing them to a settled history
  would have been a lie, and there is no honest registry sentence to add until
  ember rules on C.
- **A cut parameter follows.** market19 must be founded against a founder whose
  key is retained; the guard in §8.5 makes that structural rather than
  remembered.
- **The omission line §7 already wanted is now sharper.** Alongside *"a market
  whose resolution authority is unreachable would strand every lamport it
  holds"*, record: **a market whose founder identity is unheld strands its
  collateral and can never retire, and the protocol cannot tell the difference
  from the outside.** The predicate is not decidable from chain state the way
  §6's is — key custody is not an on-chain fact — which is precisely why it has
  to be enforced at founding.
