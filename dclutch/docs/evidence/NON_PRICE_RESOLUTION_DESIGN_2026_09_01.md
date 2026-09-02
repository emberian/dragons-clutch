# Non-price resolution — the design, and the fact nobody had checked

Architect-scholar lane, 2026-09-01. Tree root `/Users/ember/dev/dclutch`.
Measured at HEAD `64cc3436571780acdb65e78d8583328a8bbb3c34`; re-checked at HEAD
`e1fa6ade4788b8e4cb64b50bc6ee4354f819b6b8` — `git diff --stat 64cc3436 e1fa6ade`
over every file cited below is **empty**, so every line number here holds at both.

Foundation: `docs/evidence/ARCHITECT_SCHOLAR_2026_09_01.md` §A3 (Switchboard, verified),
and the ruling in `GOAL.md`: *"non-price resolution seems awesome and great."*

---

## THE HEADLINE

> **dClutch already resolves markets on a non-price fact. It has since the relayed
> family landed. The only observable that family implements is a four-state
> categorical enum — "did this token graduate?" — and there is not one line
> anywhere in the relayed path that interprets an observation as a price.**

Candidate 1 does not merely *hold*. The question it was asked — *"is the payload
price-shaped by construction, or could a relay attestor fill it with a non-price
fact?"* — has a stronger answer than either branch: **the payload is an attested
Solana account snapshot, and the one attestor built already fills it with a
non-price fact.** Non-price resolution is not a thing to design. It is a thing to
*author a second instance of*, and the authoring path has one break in it, landed
today.

Everything downstream changes. There is no new provider family to cost. The
15,000–17,000-line figure is not on the table for this question at all.

---

## §1. The load-bearing evidence: is the relayed payload price-bound?

**No. Here is every layer, in order, with the place a price would have to live.**

### 1.1 The wire — `crates/dclutch-relay-contract/src/wire.rs:48-57`

```rust
pub struct AccountObservationV1<'a> {
    key: [u8; ADDRESS_BYTES],
    owner: [u8; ADDRESS_BYTES],
    lamports: u64,
    data_len: u32,
    inline: &'a [u8],
    executable: bool,
    tail_digest: [u8; 32],
}
```

That is a Solana account as read: address, owning program, balance, full length, a
release-pinned inline prefix, the executable flag, and SHA-256 over the tail the
prefix omitted. **No price, no exponent, no confidence, no numeric value of any
kind.** The relayer signs *bytes at a slot*, and the crate's own doc comment says
why (`decode.rs:10-14`):

> *"A relayer that could sign 'the pool graduated' would be trusted with the
> proposition; a relayer that signs 'account X, owned by Y, 424 bytes, here is its
> prefix' is trusted only with the reading."*

### 1.2 The record — `crates/dclutch-relay-contract/src/record.rs`

Runtime-width, seeded by `(market, generation, account_set_id, observed_slot)`
(`:73-118`), 1-of-n fill and m-of-n seal, phases `Collecting → Sealed → Consumed →
Retired`. It carries observations **without reading them**. Capacity, from
`lib.rs:289-293`: `MAX_RELAYED_ACCOUNTS_V1 = 8`, `MAX_RELAYED_INLINE_BYTES_V1 =
448`, `RELAYED_RECORD_SLOT_BYTES = 560`, header 312. So one record attests up to
**8 accounts × 448 inline bytes = 3,584 verbatim bytes**, plus a tail digest over
everything else in each account.

### 1.3 The reader — `crates/dclutch-relay-contract/src/decode.rs`

This is the one module that reads what the record carries, and the whole table is:

```rust
pub enum RelayedObservableV1 {          // :52
    DbcMigrationProgressV1,             // :55  — the only row
}
```

`read_dbc_graduation` (`:297-330`) parses a Meteora DBC `VirtualPool` and returns
the discriminant of a **four-state enum** (`MigrationProgressV1`, `:98-112`:
`PreBondingCurve | PostBondingCurve | LockedVesting | CreatedPool`). Its declared
scale is `RELAYED_OBSERVABLE_DBC_RAW_EXPONENT_V1 = 0`
(`generated_venue_rules.rs:5`) — a bare integer, not a scaled quantity. A
non-terminal state does not return a *low value*; it refuses with
`WindowNotSatisfied` (`decode.rs:322-328`), because *"a terminal-window graduation
proposition is only ever proved by graduation."*

### 1.4 The outcome — `decode.rs:142-147`

```rust
pub struct RelayedObservationOutcomeV1 {
    observable: RelayedObservableV1,
    atoms: i128,                        // :144
    observed_unix_seconds: i64,
    observed_slot: u64,
    venue_deployment: DeploymentObservationV1,
}
```

`atoms` is a signed integer at the row's own declared exponent. Nothing names it a
price. For the graduation row it is literally `3` — the `CreatedPool` discriminant.

### 1.5 The settlement seam — `programs/dclutch-resolution-proof-sbf/src/relay_v1.rs`

`:355` `result_numerator: observation.atoms()`, `:356` `result_denominator: 1`, fed
through `:311-320` into
`SourceResolutionStateV2::resolve_primary_from_authenticated_domain(..., domain,
evidence, observation.atoms(), 1, ...)`.

### 1.6 The neutral contract — `crates/dclutch-source-contract/src/source_resolution_v2.rs:386-428`

```rust
let selector = domain.select_ordinary(numerator, denominator)?;
```

Signed rational in, ordinary selector out. `SourceSpecV1` (`lib.rs:894-901`) carries
`domain_id` and `unit_id` as **opaque `ContentId`s**. The contract states its own
neutrality at `lib.rs:3480-3486`: *"in both cases the provider bytes were
authenticated before this contract saw **a normalized integer**."*

### 1.7 The product runtime — `crates/dclutch-product-runtime-v2/src/lib.rs:222-239`

`ResultDomainV2::select_ordinary(numerator: i128, denominator: u64)` walks strictly
increasing `cuts: &[i128]` over a common `cut_denominator`. `grep -ni "price\|spot"`
over this whole 840-line crate returns **zero hits**.

### 1.8 The adversarial check — where price *is* admitted

Per the method, I looked for the accused shape being admitted before concluding it
never is. It **is** admitted — in the *Pyth* family, and only there:

- `crates/dclutch-source-contract/src/lib.rs:551-631` — `PythAdapterConfigV1
  { provider_feed_id, expected_exponent, max_confidence_bps }`, with
  `validate_update(feed, price, confidence, exponent) -> i128`.
- `lib.rs:2231-2255` — `normalize_authenticated_update(..., price, confidence,
  exponent, ...)`.

Both are **family-side**, reached only through `SourceAccessProfile::
PythTerminalOneTransaction | SharedObservationChild`. The relayed family has its own
`RelayedAdapterConfigV1` (`release.rs:305-310`: `observable_selector`,
`raw_exponent`, freshness bounds) and never touches them.

The literal string "price" appears in the entire relayed vertical **twice**, both in
doc comments, both describing Pyth *by contrast*:
`relay_v1.rs:15` and `relay_transport_v1.rs:13`.

**Verdict, stated as the prompt asked: there is no such place. The payload is an
opaque attested observation, and the price-shaped machinery lives one level down in
a sibling family that the relayed path does not call.**

---

## §2. What that makes a new non-price market cost

The generic half is already built and shipped. The measured split:

| layer | file | generic today? | marginal cost of observable #2 |
|---|---|---|---|
| relayer daemon | `tools/relayer/` (**12,759 lines**) | **fully config-driven** | **0 code; ~10 lines of TOML** |
| attestation + seal wire | `relay-contract/wire.rs` | yes (account snapshot) | 0 |
| record, PDA, quorum, phases | `record.rs`, `signature.rs`, `frame.rs`, `identity.rs` | yes (runtime width ≤ 8 × 448 B) | 0 |
| adapter config / release pin | `release.rs:305-405` | yes (selector + exponent + freshness) | 0 |
| decoding rules — **Lean** | `formal/dclutch-semantics/DClutchSemantics/RelayedVenueDecodingRulesV1.lean` (388) | head ~38 lines generic; clock block ~46 reusable | **~230** |
| decoding rules — emitted Rust | `generated_venue_rules.rs` (56) | ~13 reusable | **~30–40 emitted** |
| the reader | `decode.rs:297-330` + one enum arm + acceptance test | machinery generic | **~115** |
| interpretation orchestration | `decode.rs:356-407` | **DBC-shaped** — see §5 repair R1 | ~60–100, **once** |
| settlement seam | `relay_v1.rs`, `relay_transport_v1.rs` (2,198) | yes | 0 |
| Source contract | `source-contract` | yes ("a normalized integer") | 0 |
| product records | `ResultDomainV2` | yes (opaque coordinate) | 0 |
| **product authoring** | `partition_quality.rs`, `authoring.rs`, `market.rs` | **spot-centred — see §4** | **~250–470, once** |

**Observable #2, end to end: ~600–950 lines.** Observable #3 and onward, once §4 and
R1 are paid: **~350–480 lines.**

Against the tree's own commit-based precedents (from §A3, and re-derived here):

| | commits | insertions |
|---|---|---|
| relayed **family** | `92b137d1` + 7 siblings | 12,455 (honest: 15,000–17,000) |
| sponsored-push **profile** | `bb405b12` + 3 | 8,782 |
| **the DBC observable's own two commits** | `9fae91b3` (518) + `21b1a9a4` (559) | **1,077** — and both *created the shared machinery* a second observable reuses |

**A second non-price observable is 4–6% of a family. A third is 2–3%.** The
order-of-magnitude collapse the brief anticipated is real, and understated.

### The most decisive single number

`tools/relayer/` is 12,759 lines and contains **no DBC logic at all**. `grep -rni
"dbc\|migration_progress\|virtual_pool\|graduation"` over `tools/relayer/src/*.rs`
returns six hits, every one of them a *set name string* in a config example, a test
fixture, or a log field. `observe.rs:1` calls itself *"the observation loop for one
watched account set"*, and the set is `AccountSetConfig { name, relay_family_id,
decoding_rules_id, positions: Vec<PositionConfig> }` (`config.rs:190-201`), where a
position is `{ key, expected_owner, inline_len, admitted_data_lens }`
(`config.rs:158-173`). The worked example at `config.rs:841-867` is **under thirty lines of TOML**,
of which a position is four.

> **A new relay attestor is a config file. The daemon is already the general one.**

---

## §3. What the relayed family can and cannot reach

This is the boundary that decides whether Switchboard is needed at all.

**Reachable** — any fact a program writes into an account on a Solana cluster
whose genesis hash is pinned (`identity.rs:43-51`, `require_observed_cluster`), up to
8 accounts and 3,584 verbatim bytes per observation. That covers, with no new
family: any AMM/venue state, a governance vote tally, a token supply or mint
authority, an NFT owner, a program's deployment identity, a staking or vesting
state, another protocol's oracle account, a DAO treasury balance, whether an account
exists at all.

The family also authenticates the *observed program* cross-cluster: `decode.rs:220-256`
`require_pinned_venue` reconstructs a `DeploymentObservationV1` from the attested
Loader V3 `Program`/`ProgramData` bodies and compares ELF digest, deployment slot and
upgrade authority by exact equality. A venue redeploy mid-market makes every
subsequent observation refuse. That is a property most oracle designs simply do not
have, and it is free for every new observable.

**Not reachable** — anything that is not the bytes of an account: an election
result, a sports score, a weather reading, an off-chain API response, a court
ruling. Those need either a different attestation shape or something that *writes
them into an account first*.

That "writes them into an account first" is the whole of the Switchboard question.

---

## §4. The product side, and the break that landed today

`ResultDomainV2` is fully domain-agnostic — a categorical outcome is just its
discriminant compared against integer cuts, and `coordinate_domain_id` /
`result_unit_id` are opaque. **The wire needs nothing.** The gap is entirely in the
*authoring entrance*, and it is real.

### 4.1 The founding band assumes a spot price and a random walk

`crates/dclutch-product-compiler/src/partition_quality.rs:80-91`:

```rust
pub struct FoundingBandV1 {
    pub anchor: i128,          // ":83  Spot coordinate numerator at founding.
                               //  Must be positive: a volatility in basis points
                               //  *of spot* does not denote anything at or below zero."
    pub denominator: u64,
    pub volatility_bps: u32,
    pub window_slots: u64,
}
```

`characteristic_displacement_v1` (`:144-171`) refuses `anchor <= 0`
(`NonPositiveFoundingAnchor`, `:149`) and computes `volatility_bps × sqrt(window /
reference)` of the anchor. `PartitionQualityModelV1` has **exactly one variant**
(`:94-105`, `TriangularPlausibleBand`). For a four-state graduation enum, "volatility
in basis points of spot" denotes nothing.

### 4.2 The question vocabulary is entirely spot-relative

`crates/dclutch-product-runtime-v2-operator/src/authoring.rs:44-81` —
`MarketQuestionV1` is `ThresholdFromSpot { ticks, payout }`,
`CentredRangeProtection { profile, payout }`, `CentredBands { ordinary_cells,
profile, peak_payout }`. Every one names spot. There is no propositional shape.
`centred_cuts_v1` refuses `ordinary_cells < 2` (`partition_quality.rs:293`).

### 4.3 The graduation market is zero-cut, and zero-cut always scores 10,000 bps

`crates/dclutch-svm-harness/tests/relayed_mainnet_state.rs:184-195` states the
design, and it is right:

> *"One ordinary cell means zero cuts, and that is the honest shape rather than a
> simplification: a domain with a cut at `CreatedPool` would have an ordinary cell
> nothing could ever select, and a partition with a dead cell is a partition minting
> liabilities against an outcome that cannot happen."*

`:670` `let cuts: [i128; 0] = [];`. Under `assess_partition_quality_v1`
(`partition_quality.rs:178-247`) a zero-cut partition has one cell spanning
`[-half_width, +half_width]`, i.e. the entire band, i.e. `dominant_share_bps =
10_000`, i.e. `is_degenerate(9_000)` is true **for every possible band**. Not a
tuning problem; an arithmetic certainty.

### 4.4 THE BREAK — dated today, ancestor of HEAD

- `tools/local-validator/bootstrap/successor/src/relayed.rs:532` — the graduation
  market declares `founding_band: None`, with a comment that is *correct about the
  ethics and wrong about the mechanism*: *"it declares no belief rather than
  fabricating one it would never be measured against."*
- `tools/local-validator/bootstrap/successor/src/market.rs:3172-3180` —
  `compile_market_bodies` requires it **unconditionally**:
  `input.founding_band.as_ref().ok_or_else(|| ... "founding_band is required to
  compile this market's partition ... There is no default")`. No zero-cut branch.
- Call chain: `campaign::execute(FoundingOnly)` →
  `market.rs:1954 publish_market_records` → `market.rs:3436` → `market.rs:3465
  compile_market_bodies` → `:3172`.
- `git log -S'founding_band is required to compile this market'` returns exactly one
  commit: **`550e581b`, 2026-09-01 11:13 −0400**, *"market: the lab's fixture was a
  market nobody could lose, and now it asks something"* — 5 files, +392/−8,
  `git merge-base --is-ancestor 550e581b HEAD` succeeds.
- The recorded scenario `tools/devnet-scenarios/fixtures/graduation.json` (written
  2026-08-28, `scenarioId: graduation-four-outcome`) contains **no `founding_band`
  key anywhere in its body**.

> **This morning's partition-quality gate — a good gate, fixing a real defect —
> bricked the founding path of the only non-price market the tree has.** The
> `model.rs:104-116` doc says the intent was *"a run spec that compiles a partition
> must declare one, and one that does not compile a partition may honestly say
> nothing"* — but the graduation market **does** compile a partition (a one-cell
> one), so it takes the requirement.
>
> Verified by reading and by dating, **not** by executing the founding. See §7.

---

## §5. The recommendation

**Do not build a provider family. Build the second observable, and pay the product
entrance once.** Three named units, cheapest first.

### R1 — decompose `interpret_sealed_record_v1` off the DBC set shape (~60–100 lines)

`decode.rs:356-407` is written as generic orchestration but hardcodes the DBC
positions outside the `match observable`: `:381` `DBC_PROGRAM_POSITION_V1`, `:382`
`DBC_PROGRAMDATA_POSITION_V1`, `:383` `DBC_VENUE_POSITION_V1`, and `:275/:281`
`DBC_CLOCK_POSITION_V1` inside `require_observed_clock`. A second observable with a
different cardinality cannot reuse it. **The positions belong on
`RelayedObservableV1`, next to `set_cardinality()` (`:83-87`), which already lives
there.** This is pure refactor with an existing 19-test witness, and it should land
before anyone authors observable #2 — otherwise #2 forks the orchestration.

### R2 — the quality model becomes a family (~250–470 lines, once)

The framing that makes this small: **a founding band is a stated belief about where
the coordinate will land. A graduation market HAS a belief — "P(graduates) = x". It
is simply not a Gaussian random walk around a positive spot.** So the repair is not
an exemption for zero-cut markets and not a bypass for relayed ones. It is:

1. `PartitionQualityModelV1` gains a second variant — a stated categorical prior
   over the ordinary cells (for a one-cell partition, one probability). The existing
   `TriangularPlausibleBand` becomes the *scalar* member of a family, which is what
   its own doc already implies by calling itself *"an explicitly named modelling
   boundary ... a stated approximation with a name."*
2. `FoundingBandV1` becomes an enum (or gains a sibling `PropositionalBandV1`) so a
   categorical market states `P(true) = 3500 bps` instead of `anchor + volatility`.
   Four call sites in tools, plus tests.
3. `MarketQuestionV1` gains `Proposition { probability_bps, payout }` — zero cuts,
   coefficients `[payout, 0]`, measured against the stated prior.
4. `market.rs:3172` becomes a match on the band's kind rather than an `Option`
   unwrap. The requirement stays total: **every market states a belief; they do not
   all state the same kind of belief.**

This also closes the prior scholar's B6 hole *and* the B4/B6 width-2 tension in one
act, because a two-outcome market's honest measure is a stated prior, not a
displacement.

### R3 — author observable #2 (~350–480 lines)

Pick a fact whose account is stable and whose grammar is short. Cost is
Lean rules + emitted table + one `read_X` + one enum arm + the acceptance corpus +
~10 lines of relayer TOML. The Lean acceptance table
(`generated_venue_rules.rs:44-55`, 9 rows) and the test that walks it
(`decode.rs:430-449`, *"the Rust grammar agrees with the Lean acceptance table"*)
are the pattern to copy verbatim.

---

## §6. Switchboard, re-assessed against candidate 1

§A3's finding — Switchboard's real differentiator is arbitrary non-financial data,
and its current shape is a same-transaction quote — stands. Two refinements, both
from reading this tree:

**Refinement 1: the structural mismatch is narrower than it sounded.** The tree
already admits a one-transaction terminal shape:
`SourceAccessProfile::PythTerminalOneTransaction = 1`
(`source-contract/src/lib.rs:825-826`, *"Cheap Pyth-style terminal observation admitted
in one transaction"*), exercised at `resolution_core_v3_lifecycle.rs:862`,
`resolution_successor.rs:945`, `pre_market_resolution_funding.rs:514`. §A3 was
comparing against the *sponsored-push* profile, which exists precisely to capture
before a deadline. Profile 1 is a different, already-built answer.

What a same-transaction quote actually costs is narrower and worth saying plainly:
`relay_v1.rs:194-204` requires `observed_unix_seconds ∈ [window.start,
window.end]`. A quote fetched at settlement time carries *now* as its timestamp, so
**the settlement transaction must land inside the market's own window.** That is
operationally fine with a keeper — but there is no capture buffer, so a missed
window is a *failure outcome* rather than a delayed resolution. That is the real
price of the shape, and it is a product decision, not an impossibility.

**Refinement 2: the cheaper bridge makes Switchboard a data source, not a family —
and the brief's hypothesis holds.** A small permissionless "quote sink" program on
the observed cluster (sigverify → `verified_update` → write the result into a PDA)
turns an ephemeral quote into an account. The existing relayed family then observes
that account with **zero dClutch changes beyond one observable row** — and
`require_pinned_venue` (`decode.rs:220-256`) already authenticates the sink's ELF
digest, deployment slot and upgrade authority cross-cluster, so a sink redeploy
mid-market refuses automatically. The machinery is literally already there.

The honest cost of that: the sink is not dClutch code (~200–400 lines on the
observed cluster), and the trust story grows from two roots (relayer quorum + venue
deployment) to three (+ Switchboard's staked oracle set and the named queue §A3
warns about). **Say the third root out loud in any market that uses it.**

**Verdict: Switchboard is not a family, is not next, and is not needed for the first
several non-price markets.** It becomes interesting only when a market wants a fact
that is not on any chain, and then it arrives as *a data source feeding candidate 1*
— exactly as the brief hypothesised.

---

## §7. What I verified, and what I did not

**Verified by reading source at HEAD** (every file re-diffed clean across the HEAD
move): every file:line citation in §1–§4; the absence of price semantics on the
relayed path (`grep -ni "price\|confidence"` over `relay-contract/src/*.rs` returns
only `raw_exponent` identifiers, over `product-runtime-v2/src/lib.rs` returns
nothing, over `relay_transport_v1.rs` returns one comparative doc comment); the
relayer's venue-independence; the commit-based cost table via `git show --shortstat`;
`550e581b`'s date, content and ancestry via `git log -S` and `git merge-base`; the
graduation fixture's missing `founding_band` by parsing the JSON.

**Counted, not executed**: `relayed_mainnet_state.rs` has **19** `#[test]`/
`#[tokio::test]` attributes at HEAD. I did **not** run them. "19/19 on real ELFs" is
inherited from the session record, not re-measured here.

**Traced, not executed**: the founding refusal in §4.4. I followed
`relayed.rs:532 → validate_market_input → campaign FoundingOnly → market.rs:1954 →
:3436 → :3465 → :3172` by reading, and corroborated it with the commit date and the
band-free fixture. **I did not run a founding and watch it refuse.** That is the one
task this document leaves open, and it is cheap: found the graduation market against
a local validator and read the error. If it *succeeds*, some path I did not find
supplies a band, and §4.4 is wrong in mechanism though not in shape.

**Not re-verified**: every Switchboard fact in §6 is §A3's, inherited. I re-read no
external documentation this session.

**Line-count estimates in §2** are reasoned from module structure, not from a landed
diff. The commit-based figures (1,077 / 8,782 / 12,455) are real; the
600–950 and 350–480 are engineering estimates and should be read as such.

---

## §8. What would have to be true to choose otherwise

- **To build a Switchboard family anyway**: a market must need a fact that reaches no
  chain, *and* the three-root trust story of a sink must be judged worse than a
  fourth family's 15,000–17,000 lines. Both halves have to hold. Today neither is
  established.
- **To treat non-price as unbuilt**: §1 would have to be wrong — some reader would
  have to interpret `atoms` as a price. `RelayedObservationOutcomeV1` is referenced
  in exactly one file outside its own crate (`relay_v1.rs:33,127,272`), the atom has
  exactly two consumers (`relay_v1.rs:318` into the transition, `:355` into the
  certificate), and the outer that holds the plan reads only `plan.next_source` and
  `plan.certificate` (`relay_transport_v1.rs:1009,1011`) — never the observation. If
  someone finds a third consumer, this document is wrong and worth reopening.
- **To skip R2 and exempt zero-cut markets from the gate instead**: that would say a
  proposition market has no belief to state. It has one; it is the price of the
  contract. Exempting is cheaper by maybe 150 lines and buys a partition nobody
  measured — which is the exact defect `550e581b` landed to fix.
- **To skip R1**: acceptable only if observable #2 has the identical four-position
  shape (program, programdata, state, clock). If it does, R1 can wait for #3. If it
  does not, skipping R1 forks the orchestration and the family stops being one.

---

## §9. What the build lane executed — 2026-09-01, added below the scholar's own text

Nothing above is edited. This section records what happened to §5's three units
and to the one task §7 left open, so a reader of this file does not have to
cross-reference the log.

**The open task, run.** §7 says: *"I did not run a founding and watch it
refuse."* It was run first, before anything was built, by driving the relayed
graduation market's own `MarketRunInput` through `compile_market_bodies` — the
first act of `publish_market_records`, before any RPC, so the exact site the
founding campaign reaches. Verbatim:

> `founding_band is required to compile this market's partition: state anchor,
> volatility_bps, window_slots, plausible_half_widths and max_cell_share_bps.
> There is no default -- volatility is an authoring input, and a partition
> cannot be measured for degeneracy without the belief it is meant to describe`

**§4.4 was right in mechanism as well as in shape.** No path supplies a band.

**R1** (`0b8c377d`): the DBC positions moved onto `RelayedObservableV1` as
`RelayedSetLayoutV1`. Behaviour-preserving, proven by the shipped
relay-consuming ELF rebuilding byte-identical
(`f64190e2e257c273387852560323a9d688c3e323a8ca49939fa474ac6e49b4f8`) with both
crates genuinely recompiling, and by the nineteen `relayed_mainnet_state` tests
— **counted in §7, and now run**: 19 passed on real ELFs.

**R2** (`cbf983fe` compiler, `26179076` founding path): the belief became a
family, `FoundingBeliefV1`. §5's framing held and made the unit small. Two
corrections to the shape §5 proposed, both in the direction of fewer authors:

- The belief and the model are **one type**, not two. §5.1 and §5.2 proposed a
  second `PartitionQualityModelV1` variant *and* a band sibling; carrying the
  parameters on both makes a mismatched pair representable and then owes
  somebody a check that they agree. `PartitionQualityModelV1` is now the *name*
  a report carries and the parameters live on the belief.
- `MarketQuestionV1::Proposition` carries **only a payout**, not §5.3's
  `probability_bps`. The probability is the belief; stating it in the question
  as well would be two authors on one number.

The zero-cut hole closes as §5 predicted, and one thing §5 did not name closes
with it: `unresolved_share_bps`. A proposition's unproved mass lands on the
Product's own disclosed failure outcome, which is not an ordinary cell, so a
market believed at 500 bps is refused as degenerate *from the failure side* —
something no measure over ordinary cells alone could ever have seen.

**R3** (`1fe58874`, `871017cf`): observable #2 is SPL Token-2022 mint-authority
renunciation. §2's estimates measured against the landed diff:

| §2's estimate | landed |
|---|---|
| decoding rules — Lean ~230 | +344 (including per-row layout generalization and eleven new theorems) |
| emitted Rust ~30–40 | **+35** |
| the reader ~115 + one enum arm | +313 in `decode.rs`, of which ~180 is the acceptance corpus and the two-row distinguishability tests |
| relayer TOML ~10 lines | **23 non-comment lines, no code** |
| interpretation orchestration | **0** — R1 had already paid it; the spine took one arm |

**§8's R1 clause, answered honestly.** *"To skip R1: acceptable only if
observable #2 has the identical four-position shape."* It does. R1 was
therefore not strictly required for #2 — what it bought here is that the
positions are **per-row data** rather than module constants, and that the state
position's pinned inline width comes off the row (424 against 82) instead of
being typed twice. A row with a different *cardinality* would exercise it
fully, and none exists yet: the natural shape of "a fact a program wrote into
an account" is one state account plus the three the family always needs. The
first proposition that would want a fifth is a **conjunction over two accounts
of the same venue**, and no market has asked for one.

**The route is witnessed, not only proven.** `relayed_mainnet_state` is 19 → 24
on real ELFs: row 1 is driven create → append ×4 → seal → consume → resolve
through the same transport, quorum, funding, deadline walk and settlement, with
nothing changed but the adapter's `observable_selector` and the grammar it
selects. Including the sharp one on chain rather than only in Lean: a zeroed
82-byte account reads as `COption::None` in both tags, a real quorum signs those
real bytes, and the adapter refuses — with the positive control in the same run,
because the same bytes with `is_initialized` set to one do resolve.
