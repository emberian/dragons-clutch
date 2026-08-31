# Direct Hot: where the variance is, where the mass is — 2026-08-30

**Superseded in part by `30574297` (same day):** the CoreState carry this doc
measured as inert is now engaged in the fixture. Current figures there: floor
1,318,826 (constant 1,320,326), worst seed 1,345,829, band 16,501, survivor
sites 7 (the three Market sites left the key-varying term), tail P(key >
1,400,000) = 1 in 1.10 billion at p=½ (1 in 13.9 million at the empirical
p̂=0.446). The realm invocation count in §survivors is also corrected there:
one Custody CPI per trade, not two — realm is worth 9,000, not 18,000. The
MASS anatomy and the gate's floor semantics below stand unchanged.

**Superseded again by ALLKEYS (`308c3dff`..`e7805d62`, same day): §4's whole
variance table is now historical.** The surviving key-varying searches are
**zero**, not seven — every remaining bump is carried in the V3 envelope's
eight already-reserved bytes at offset 120, so the wire stays 1,167 B and no
pin moved. **Every key costs the same 1,336,742 CU (63,258 of margin) and
`P(refuse)` on a real Market is exactly 0.** §4's `P(a stranger's key exceeds
the ceiling) = 0.032%` was additionally a fixture artifact — the same
unre-derived staged bumps FIXBUMPS found — with real pre-lane Markets at 1 in
34.9 million. What survives from §4 is its *method*: a measured worst is a
sample, not a bound, while any key-varying search remains, so the gate asserts
the key-independent floor and the cut reports floor plus tail. The MASS anatomy
in §5 is unaffected: searches were 1–2% of mass and 100% of variance, so
removing them moved the variance and not the total.

Two questions, one measurement campaign, two commits.

1. **The variance.** Which `find_program_address` searches still move with a
   participant key, what each one costs, and what a margin gate can honestly
   assert about them.
2. **The mass.** Where the ~1,350,000 CU of a public Direct trade actually goes,
   so that a choice between the AOT accelerator, seal/cache amortisation and a
   two-transaction lifecycle is made against numbers.

Every figure below was measured on this laptop from a clean working tree. The
two ELF sets are named at every comparison, because the whole first half of this
document is about how badly a CU figure travels between builds.

## 0. The result that reframes both questions

Between `ff543148` and `9dbbc371` the public Direct route's worst swept seed fell
**1,390,745 → 1,363,745** and its cross-seed band collapsed **52,500 → 24,000**.

The route's key-independent cost over that interval changed by **1 CU**.

| statistic | `ff543148` | `9dbbc371` | moved by |
|---|---:|---:|---:|
| worst of 32 seeds | 1,390,745 | 1,363,745 | **−27,000** |
| mean of 32 seeds | 1,355,639 | 1,350,245 | −5,394 |
| cross-seed band | 52,500 | 24,000 | −28,500 |
| Registry reauth pair | 52,592 | 52,592 | 0 |
| Claims constant part | 144,911 | 144,910 | −1 |
| Custody constant part | 130,858 | 130,858 | 0 |
| **key-independent floor `C0`** | **1,321,743** | **1,321,742** | **−1** |

The intervening commits are `557df0d1` (splitting a stack frame in
`direct_replay_setup_v1`, which retired the seven SBF frame-overwrite
diagnostics), `ee3dbe8f`, `e164feda`, `3ac3c0bf`, `9dbbc371`. None of them
touched the route's arithmetic. What they touched was the Trading ELF — and the
Trading ELF's digest is an input to `release_set_id`, which is a seed of the
Market identity, which is a seed of almost everything below it. So every bump
depth on the route was redrawn, and the route drew better.

The old gate constant, `worst swept seed <= 1,387,000`, was RED by 3,745 at
`ff543148` and GREEN by 23,255 at `9dbbc371`. It measured a die roll.

### The related claim that is false, and worth retiring

The gate and `DIRECT_HOT_BUMP_CARRY_DESIGN_2026-08-30.md` both say a **rebuild**
redraws the lottery "with no source change at all". It does not. Building the
five role ELFs twice from the same clean `ff543148` tree reproduced the entire
32-seed sweep to the compute unit — min 1,338,245, worst 1,390,745, mean
1,355,639, activation-cache bump 252 — and CARRY's independently-taken sweep at
the same commit agrees to the unit. The link is deterministic. What redraws the
lottery is a **source change to any of the five roles**, which is a stronger and
more useful statement: it means a lane can rebuild to re-measure, and cannot
compare across a patch.

## 1. The census: ten key-varying search sites over eight addresses

Method. Two static reachability traces of the top-level Direct route (Trading;
Claims `sparse_native_transfer_v1`; Custody delegated Transfer), then an
*empirical* identification that does not depend on them: derive every candidate
PDA from the fixture's own planted accounts at each of the 32 seeds, and fit the
per-CPI compute accounting against those depths. A census is complete when the
residual stops moving with the keys.

It does. Fitted at `ff543148`:

| program | model | residual across 32 seeds |
|---|---|---:|
| Registry ×2 | no key-varying search | **0** (26,296 CU each, identical on all 32) |
| Claims ×1 | `market` | spread **10 CU** |
| Custody ×1 | `market + custody-replay + custody-transfer-authority` | spread **32 CU** |
| Trading own | `market + root + 2 maker replays + Custody caller authority + Claims caller authority` | spread **4,500** = exactly the unmodelled site |
| whole transaction | all ten sites | **spread 142 CU over 32 key draws** |

`CU(seed) = C0 + 1,500 × T(seed)`, and `C0` is flat to 142 CU (0.01%) at
`ff543148` and 33 CU at `9dbbc371`. Two seeds carry a genuine non-search
key-dependent term (+140 and +4 CU); everything else is bump depth.

### The survivors

| # | address | sites | site locations | seed composition | class |
|---|---|---:|---|---|---|
| 1 | Core Market state | **3** | Trading `hot_v3.rs:10398`, Claims `sparse_native_transfer_v1.rs:518`, Custody `lib.rs:509` | 9 seeds; the payer enters transitively — `capability_manifest = hash(manifest)`, the manifest carries the config digest, and `DirectExecutionConfigV1`'s `fee_recipient` is the payer. `release_set` is seed 7. | **(a) carrier exists and is INERT** — see §2 |
| 2 | Direct capability root | 1 | Trading `dispatch.rs:318` | `[domain, market, generation, manifest, entry_index, kind, capability_release, config]` | (c) irreducible today: the root's own address is what is being authenticated, and no account upstream of it carries its bump |
| 3 | seller maker replay | 1 | Trading `hot_v3.rs:6292` (preplan pass only) | `[domain, market, generation, seller]` | (b) migration class — `DirectMakerStateV1` would have to record its own bump; the replan pass already reuses the preplan bump, so this is one search, not two |
| 4 | buyer maker replay | 1 | Trading `hot_v3.rs:6292` (preplan pass only) | `[domain, market, generation, buyer]` | (b) same |
| 5 | Custody caller authority | 1 | Trading `child_authority_v4.rs:65` | `[domain, release_set, market, role, context, hash(child request)]` | (c) irreducible: the bump is in its own seeds through the request digest. `a22b7355` carried the CHILD's re-derivation; the CALLER must still search once to produce the byte it carries |
| 6 | Claims caller authority | 1 | Trading `claims_composition_v3.rs:162` | `[domain, release_set, market, role, request_id, hash(packet)]` | (c) same |
| 7 | Custody replay | 1 | Custody `lib.rs:787` | `[domain, market, release_set, role, context]` where context is the buyer's maker-replay root | **(b) migration class, and the size is ONE BYTE** — `CUSTODY_REPLAY_BYTES_V1 = 288` is exactly packed, Lean-emitted, `exact_physical_widths` asserts 288, `require_header` refuses any other width. Growing it orphans every replay on chain |
| 8 | Custody transfer authority | 1 | Custody `lib.rs:1336` | `[domain, market, release_set]` | (b) migration class — the natural carrier is the Custody replay account, which is the same 1-byte problem as #7 |

**Ten sites, eight addresses, and the Market is three of the ten.** That
multiplier is why the Market draw dominates: one unlucky Market bump is paid
three times.

Reconciliation against the prior census (`DIRECT_HOT_PDA_SYSCALL_AUDIT_2026-08-28.md`,
40 searches on the one-Custody fixture). CARRY removed 20 sites (`395210c9`,
`a40ef689`, `e0a2fd25`, `a22b7355`, `490900be`, `a0cba859`); of those, the six
activation-cache sites, the three Claims own-account sites and the one capability
seal are genuinely gone from this route, and the three Market sites and the
realm pair are gone **from the code but not from this fixture** (§2). The rest of
the 40 are the constant class: 8 product-graph record searches in Claims, 8 in
Trading, 2 execution-strategy, 2 realm — none of which moves with a key, all of
which is measured by `direct_hot_record_depth_census.rs`.

### Cost, per site, on each build

Attempts, not CU, because CU is `1,500 × attempts`. Range is over the 32 seeds.

| site | `ff543148` min–max | `9dbbc371` min–max |
|---|---|---|
| Market (×3) | 1–10 | 1–4 |
| Direct root | 1–7 | 1–7 |
| seller replay | 1–10 | 1–5 |
| buyer replay | 1–9 | 1–6 |
| Custody caller authority | 1–7 | 1–6 |
| Claims caller authority | 1–4 | 1–4 |
| Custody replay | 1–9 | 1–5 |
| Custody transfer authority | 1–4 | 1–7 |

Pooled over 224 measured draws per build, the mean attempt count is 2.241 and
1.879, so `p̂` is 0.446 and 0.532 — straddling the theoretical ½ (a random
32-byte value is a valid Ed25519 point about half the time). The model is
`Geometric(1/2)`, and the two builds together are 448 draws of evidence for it.

## 2. The finding a lane should act on: the CoreState carry is inert in the fixture

`programs/dclutch-trading-sbf/program-test/direct-hot/src/fixture.rs:673` stages
the market with `bumps: StateBumpsV1::UNRECORDED`. `git log -L` says `e93fe5e9`
(CoreState phase A) added that literal to keep the fixture compiling, and nothing
since has changed it. Zero means search, by phase A's own design — so on this
fixture all three Market readers take the `find_program_address` fallback and
Custody's `authenticate_realm` searches for the realm raw/staging pair. The
carriers landed by `a0cba859` are **dead code on the route the gate measures**.

Three independent confirmations: the literal itself; the empirical fit (Claims
has exactly one key-varying search and it is the Market, which is only possible
if the Market is searched); and the Trading reachability trace, which resolves
`match state.bumps.market` to the `None` arm.

The contrast is what makes this an oversight rather than a decision. The same
lane series deliberately staged the other three carriers the way a deployment
produces them, with a comment saying why — `waist.rs:545`
`put_activation_cache_bump_v1`, `fixture.rs:703`
`put_liability_basis_market_bump_v2`, `fixture.rs:762`
`put_liability_basis_position_bump_v2`: *"A fixture that left them zero would
stage accounts no deployment produces and would measure a route nobody runs."*
The CoreState tail is the one carrier that did not get that treatment.

**Sized.** Recording the three bumps is a fixture change with no route code in
it. It deletes the largest single key-varying term — one draw multiplied by
three sites — and 9,000 CU of constant realm cost (the pair at bumps 253/251 is
8 attempts = 12,000 CU, carried 3,000). On `ff543148`'s worst seed the Market
draw alone was 10 attempts × 3 sites = 45,000 CU, of which 40,500 would have
gone. It also drops the key-varying site count from ten to seven and the
Market's multiplier from three to zero.

Until it is done, **no number taken from this fixture may be quoted as evidence
that the CoreState carry saved anything.** In particular `direct_hot_record_depth_census.rs`
marks the realm row `CARRIED` and prints an 18,000 CU saving; on this route it is
neither carried nor 18,000 — see §3.

## 3. Two more corrections to numbers already written down

**The route makes ONE Custody CPI, not two.** 192 program invocations over 32
seeds is exactly six per transaction: Registry, Registry, Claims, Custody, Token
(nested in Custody), Trading. The reason is arithmetic in the fixture:
`gross = FILL × EXECUTION_PRICE / PRICE_SCALE = 10 × 50 / 100 = 5` and
`fee = 5 × 50 / 10_000` floors to **zero**, so `seller_terminal` is the only
enabled register and the seller-intermediate/fee-continuation pair never fires.
The 2026-08-28 audit said the same thing in words ("the shipped fixture has
Claims plus one seller-terminal Custody route"); the record-depth census
(`invocations = 2`, `direct_hot_record_depth_census.rs:232`) and `a0cba859`'s
commit message say the opposite. On this route the realm pair is worth **9,000
CU, not 18,000**.

**The gate measures a fee-free trade.** A fee-bearing Direct trade enables two
Custody routes instead of one. A second Custody CPI costs on the order of the
first — 135,358 to 151,858 CU measured, plus its caller-side CPI charge — so a
fee-bearing trade lands around **1.49–1.52 million CU and does not fit under
1,400,000**. That is an estimate from the measured cost of the one route that
does run, not a measurement of the two-route shape, and it deserves its own lane.
It is the single largest thing this campaign found that nobody is tracking.

**The Registry reauthentication pair is 52,592 CU, not 55,513.** Identical on all
32 seeds at both commits (26,296 + 26,296). HEAPRED measured 27,757 + 27,756 =
55,513 at `3dde1b9c` on the continuation route. The 2,921 difference is not a
multiple of 1,500, so it is not bump depth: it is code, between two commits.

## 4. The gate policy, re-derived

### What cannot be done, stated as a refusal

**No constant can bound a stranger's key while the searches remain.** Bump depth
is `Geometric(1/2)`: the probability that a search needs `k` attempts is `2^-k`,
unbounded above. The per-site maximum over 32 draws is not a bound either — it is
itself a draw, and it moved **1,441,743 → 1,399,742** between two commits whose
code cost differed by 1 CU. The charter's "analytic worst = per-search max-depth
sum + measured floor" is computed and printed by the gate, but it must not be the
constant, because a green there would mean "this sweep drew well".

CARRY's correction stands and this is its general form: `create_program_address`
is itself 1,500 CU, so converting a search of depth `d` saves `(d−1) × 1,500` and
nothing at `d = 1`. There is no positive depth-invariant floor for a conversion —
and symmetrically, no depth-invariant ceiling for a search.

### What can be done: gate the key-independent cost

`CU(seed) = C0 + 1,500 × T(seed)`. `C0` belongs to the code; `T` belongs to the
keys. The gate is on `C0`.

The statistic is `min over seeds of (CU(seed) − 1,500 × T_known(seed))`, where
`T_known` sums the nine sites the test reproduces from the fixture's own planted
accounts. That minimum equals `C0 + 1,500 × k`, where `k` is the tenth site's
depth on the luckiest of the 32 draws; `k = 1` unless all thirty-two draws missed
on their first candidate, probability `2^-32`. It is therefore a bound on a
property of the code, and eighteen of the thirty-two seeds attain it.

**The arithmetic, at `9dbbc371`:**

```
floor statistic (measured)                     1,323,242
  less the one attempt the tenth site makes       −1,500
implied C0                                     1,321,742
  plus ten sites at one attempt each            +15,000
a route whose every search landed first try    1,336,742

gate constant = floor + one bump attempt       1,324,742
```

One attempt of slack, because 1,500 CU is the smallest unit this route can spend.
`df404c56`'s 7,520 CU regression would have gone red five times over. The 142 CU
of measured non-search jitter fits inside it with room.

**Cross-build validation, which is the whole argument:** run the same gate
against both ELF sets. `ff543148` → floor 1,323,243. `9dbbc371` → floor
1,323,242. The statistic the old gate asserted moved 27,000 CU across the same
pair.

### What the gate now says about fitting, since it no longer pretends to

With `C0 = 1,321,742` and ten `Geometric(1/2)` sites (the Market counted three
times), the exact distribution gives:

| threshold | P(a uniformly drawn key exceeds it) |
|---|---|
| 1,400,000 (protocol ceiling) | **0.032%** — about 1 public trade in 3,100 |
| 1,387,000 (the old gate constant) | 0.257% |
| median | 1,349,936 |
| 99th percentile | 1,378,436 |
| 99.99th percentile | 1,408,436 |

This number is **identical at both commits**, because it depends only on `C0` and
the site inventory. It is the honest acceptance criterion and it is the one thing
in this document that does not move when the ELF does. At the conservative fitted
`p̂ = 0.446` it is 0.158%, about 1 in 630; the true value is between.

Deleting the three Market sites (§2's fixture fix plus a founding that records
the bumps) takes the site count from ten to seven and `P(refuse)` from 0.032% to
under 0.002%.

## 5. The mass: where 1,350,000 CU goes

Instrument: the in-tree `hot-cu-profile` Cargo feature, which turns ~26
`hot_cu_checkpoint!` sites into `sol_log_compute_units` calls. Trading built with
the feature at `9dbbc371`; the other four roles are the shipped ELFs. Same
fixture draw both ways: **plain 1,346,936 CU, profiled 1,375,228 — instrumentation
load 28,292 CU, 2.10%**, measured rather than assumed. Per DECOMP's standing
warning the profiled build also takes the extended-heap arm, so this table is
valid for ATTRIBUTION and must never be read as a shipped total.

Buckets sum to 1,375,003 of the 1,375,228 CU transaction — 225 CU unattributed,
0.016%.

| bucket | CU | % |
|---|---:|---:|
| **child role CPIs — the economic work** (Claims 149,412 + Custody 139,858 incl. Token 112) | 289,270 | 21.0 |
| **effect projection** (transition VM / effect kernel) | 164,289 | 11.9 |
| **register projection** (account, rent-quote, native-signature, request) | 152,204 | 11.1 |
| **commit** (lifecycle closes 116,815 + non-root 4,118 + root 3,675) | 124,608 | 9.1 |
| **Market + Direct root + product-graph record authentication** | 109,139 | 7.9 |
| **runtime observations** — account-profile projection of the 57-account frame | 95,173 | 6.9 |
| lifecycle creates (6 System CPIs) + child-walk bookkeeping | 82,532 | 6.0 |
| lifecycle preplan, candidate, replan | 74,265 | 5.4 |
| preflight: role programs, invocation resolution, child preflight | 63,499 | 4.6 |
| local effect discipline + child composition | 54,949 | 4.0 |
| **Registry reauthentication CPIs ×2** | 52,592 | 3.8 |
| sealed artifacts + execution-strategy record + Effect decode | 50,761 | 3.7 |
| effect permissions + discipline banks | 35,829 | 2.6 |
| entry, dispatch, hot-invocation authentication | 20,966 | 1.5 |
| geometry / rent quotes + sealed-ownership arena | 4,927 | 0.4 |
| **total** | **1,375,003** | **100.0** |

The three coarse facts:

* **Trading's own code is 75% of the transaction** (1,003,574–1,030,574 CU across
  the sweep at `ff543148`). Any lever that does not attack it is rearranging the
  other quarter.
* **PDA search depth — the entire subject of §1 — is 15,000 CU at the floor and
  about 30,000 at the mean.** It is 1.1% to 2.2% of the transaction. The variance
  it causes is four times the *margin*, which is why it matters, but it is not
  where the money is.
* **Interpretation and projection is the money.** Effect projection, register
  projection, runtime observations and composition together are 466,615 CU, 34%.

### What each candidate lever attacks

| lever | buckets | CU | share |
|---|---|---:|---:|
| **two-transaction lifecycle** (children and commit out of line) | child role CPIs, lifecycle creates, commit | 496,410 | 36.1% |
| **AOT accelerator** (precomputed projection replacing interpretation) | effect projection, register projection, runtime observations, composition | 466,615 | 33.9% |
| **seal / cache amortisation** (authentication reuse across trades) | Market+root+record authentication, sealed artifacts + strategy + Effect decode | 159,900 | 11.6% |
| deleting the ten key-varying searches | bump depth | 15,000–78,000 | 1.1–5.7% |

Nobody has measured the AOT accelerator on this route in either direction —
`programs/dclutch-direct-aot-sbf/tests/program_test.rs` captures `accepted_cu`
and only prints it, and `OMISSION_INDEX.md` O-004 / U-014 still name the
comparison as unbuilt. That is a one-lane measurement standing between this table
and a decision, and it is the largest single unknown on the list.

## 6. What was NOT verified

* The **two-Custody fee-bearing shape** was not executed. The 1.49–1.52M figure is
  arithmetic on the measured cost of the one-route shape.
* The **Claims caller-authority depth** is inferred from the residual, not derived:
  its packet digest is the one seed no public fixture field carries. The
  inference is that the residual is a non-negative multiple of 1,500 with a
  geometric distribution, which it is on both builds.
* The **`p = 1/2` model** is theory plus 448 pooled draws; the two builds' point
  estimates are 0.446 and 0.532, and the refusal share is quoted as a range for
  that reason.
* The profiled phase table is **one fixture draw**, not a sweep, and comes from an
  instrumented build in a different heap regime. Attribution only.
* The **fixture fix in §2 was not made** — this lane may edit only the margin gate
  and evidence documents. It is sized, not landed.
* No devnet writes, no route code changed, nothing under `formal/`.
