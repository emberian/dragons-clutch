# Architect-scholar reading — 2026-09-01

Status: reading, not a ruling and not a repair. Owner: the architect-scholar lane.
Measured across `61817d7a` → `b6b14ab2` on `/Users/ember/dev/dclutch` (the tree moved
under this lane as other lanes committed; every measurement below was taken with
`git rev-parse --show-toplevel` and `git rev-parse HEAD` printed in the same command,
and none of them is HEAD-sensitive in the range).

Each item lands in exactly one of three states, per
`docs/MASTER_COMPLETION_CONTRACT.md`: **DISSOLVED** (a fact removes the question),
**DECIDED** (evidence forces the answer), or **EMBER'S** (the minimal framing plus the
real cost basis, because no reading can settle it).

**Six of the thirteen dissolved.** Two of the coordinator's decisions are wrong and one
of ember's four is not a question. The governing pattern held: the ruling-shaped items
were where unread code was hiding, because nobody audits a sentence saying a human must
decide.

## Verdicts at a glance

| item | verdict | the fact that settles it |
| --- | --- | --- |
| **A1** LP consent floor | **DISSOLVED** | the config digest is a capability-root PDA seed, so a raised floor is a different pool at a different address |
| **A2** Recovery ontology | **EMBER'S** | a silent source is already survivable (`CommitDeadlineFailure`, witnessed); recovery buys resolution *quality*, not liveness |
| **A3** Provider-family scope | **EMBER'S** (breadth) / **DECIDED** (refactor) | Switchboard's current path is same-transaction quotes, structurally unlike the profile dClutch has; the refactor's stated reason was already fixed at `01478abe` |
| **A4** Claims split/merge | **DECIDED — build** | unreachable by all four candidate compositions; the contract is green and already has an operator |
| **B1** Materialize/Dematerialize | **DECIDED — cut, but cut the route** | two of four grounds false; the codec is not on the admitted route |
| **B2** Two `dclutch` binaries | **DECIDED — and BACKWARDS** | the Rust one is the published artifact with downloads; the TS one is `private: true` and 404 |
| **B3** The heap | **DISSOLVED to a measurement** | `DCLTHOT3`'s justification describes CPIs that decision 0017 deleted the same day |
| **B4** K = 2 usefulness | **DISSOLVED**, and K = 2 is not a harbour either | a K = 2 spline record cannot decode at all; and K = 2 fits by 3 bytes, which the house packet builder spends |
| **B5** 9000 cell-share ceiling | **partly EMBER'S — the dissolution is wrong** | the constant has no production reader *and* caps nothing: authors may state up to 10,000 |
| **B6** Zero-cut markets | **CONFIRMED for relayed, wrong reason, plus a live break** | relayed mints heavily; and it is not ungated, it is unfoundable |
| **C1** Mixed-unit solvency gate | **DECIDED** | scale's owner is `ProductBasisV3::payout_scale`; `CoreState` has no scale field |
| **C2** Empty satisfying set | **DECIDED** | `Content`/Route is upstream, so the never-admitting guard comes out last, with a control |
| **C3** C-16 completion | **DECIDED** | already operational in the entry list; its ruling table is stale in its own §3 category |

---

---

## A1. The LP consent floor — **DISSOLVED**

**A raised `locked_capital_floor` is not a policy evolution. It is a different pool at a
different address.**

The chain of facts, each checked:

1. `locked_capital_floor` lives at byte offset 112 of the 128-byte `DealerConfigV4`
   (`crates/dclutch-dealer-codec/src/config_v4.rs:26,48,126`).
2. `programs/dclutch-trading-sbf/src/dealer/mod.rs:169` refuses `Content` unless
   `hash(config_bytes) == context.selection().config()`.
3. That `selection` is read out of the **on-chain root header**, not supplied by a
   caller: `TradingFamilyContextV1::authenticate_at`
   (`programs/dclutch-trading-sbf/src/dispatch.rs:310-330`) decodes
   `CapabilityRootHeaderV1` from the account data and then re-derives the account's own
   address from that header's seeds — *"the derivation IS the check"* (`:299-303`).
4. `CapabilityRootSeedsV1::as_slices`
   (`crates/dclutch-capability-program-contract/src/lib.rs:759-778`) puts **`config`
   itself in the capability-root PDA seed list**:
   `[domain, market, generation, manifest, entry_index, kind, capability_release, config]`.
5. The LP position is a PDA under that root —
   `[DEALER_LP_POSITION_PDA_DOMAIN_V3, child_root, lp_owner]`
   (`v3_multi_lp.rs:265-271`) — and `authenticate_lp_position` pins
   `lp.child_root != context.child_root` (`:1064`).

So raising the floor changes the config bytes → the config digest → **the child-root
address**. The new root is a new, empty pool; the LP's capital and position remain under
the old root, whose config still carries the floor they joined under. **There is no route
by which the floor an LP was admitted under can change beneath them**, and therefore no
consent question of the shape the ruling describes.

**Every production filler of the floor is the authenticated config.**
`v3_accelerator_accounts.rs:308,328` and `v4_equity_accelerator_accounts.rs:1026,1045`
all read `config.locked_capital_floor()`. `v4_equity_accelerator_accounts.rs:384` refuses
unless `config.locked_capital_floor() != request.locked_capital_floor` is false;
`v3_equity_operator.rs:536,628` pin request against chain, and that field's own doc at
`:205` already calls it *"Immutable scenario residual floor."* The three `0` literals
(`v3_trade.rs:1264`, `v4_scenario_operator.rs:214`,
`v4_equity_accelerator_accounts.rs:1934`) are each inside a `#[cfg(test)]` module —
checked, not assumed.

**What the measurement actually measured.** `dealer_v3_two_lp_life.rs:388` calls
`plan_pool_equity_v3` directly with `locked_capital_floor` as a free parameter. Floor 0
pays, 60 refuses, boundary exact at 20/21 — all true, and all true of the *kernel
planner*, where nothing binds the floor to a PDA. The test's own docstring
(`:377-379`) states the premise that does not hold: *"a policy parameter carried by the
selected immutable descriptor"* — correct — *"so an LP who joined under one floor … can
be refused its exit by an evolution it never agreed to"* — which requires the descriptor
to be able to change under a live root, and it cannot.

### The candidate repair rests on a false premise, and would have been built

*"Evaluate a position against the generation it was admitted under; raising the floor
structurally IS a new generation; rolling forward is the consent."*

**`generation` is not an epoch. It is a component of the market's immutable identity.**
`MarketIdentity.generation` (`crates/dclutch-core-contract/src/lib.rs:130-141`) is
caller-supplied once at founding (`programs/dclutch-core-sbf/src/found.rs:170,404`) and
has no setter and no increment anywhere in the tree. `MarketRoot` only ever
`require_generation`s it (`:401,425,443,455-461`); the doc at `:130-131` says so:
*"The generation is part of the identity itself."* Advancing it is not an operation —
it is founding a different market.

`DealerLpPositionV3.generation` (offset 224, written at `prepare_lp_open_v3:288`, checked
at `:1068`) is therefore **a market-identity binding, not an admission pin**. Building
"evaluate against your own generation" on it would have added per-position state to
express something the address already expresses.

### The general invariant, already enforced structurally

> **A position's address commits the terms it was admitted under.**

"Current selection" and "the position's own selection" are the same object by
construction, because the position's PDA descends from a root whose PDA contains the
config digest. There is no second site to patch and no invariant to state — only one to
*write down*, because it is currently discoverable by nobody except by reading
`CapabilityRootSeedsV1::new`.

### What the floor guards, and the hole that is real

`residual_at` → `if after < input.locked_capital_floor { Insolvent }`
(`v3_equity.rs:280,377`): the minimum per-scenario poststate residual. That is the same
scalar as C1's mixed-unit defect below, so **the floor's guard is only as sound as C1**.

### C-06's "consent-safe policy/epoch evolution" — answered, and nobody has said so

`epoch` has **zero occurrences** in the entire Dealer stack, and there is no
`evolve`/`Evolve`/`reselect` route anywhere in the tree. The architecture's answer is the
one above: evolution *is* a new capability root, and the old pool stays drainable by its
own LPs under its own terms. That is consent-safety in its strongest form — the old terms
are not merely honoured, they are unreachable from the new ones. **C-06's clause is
satisfied structurally; the work is a campaign that demonstrates it plus one paragraph
saying so, not an evolution instruction.**

### The change that CAN freeze a live position — already ruled, already disclosed

`CoreSbfError::ReleaseSuperseded = 0x3010`
(`programs/dclutch-core-sbf/src/lib.rs:135-141`): *"The release's pinned deployment slot
moved: the substrate was upgraded. **Every open market on the superseded release
generation refuses until a re-release re-authenticates the new deployment and re-pins its
slot.**"* That is a real "terms I never agreed to" event, it applies to every market and
not just Dealer, and its trigger is whoever holds the upgrade authority.

It needs no new ruling: it is decision 0012, ruled by ember 2026-08-27, and it is
disclosed in the release manifest by name rather than left to be inferred — the evidence
class is literally
`slot-pinned-release-set-with-a-retained-upgrade-authority` versus
`immutable-release-set` (`crates/dclutch-release-tool/src/infrastructure.rs:505-516`).
Recorded here because it is the honest answer to the question A1 was reaching for, and
because the mutable class was re-chosen for the current cohort (GOAL.md, deploy section)
on rent-recovery grounds.

---

## A2. Recovery ontology — **EMBER'S**, and the question is not the one on the contract

### Plain English first

It is not magic crisis money. No pot appears and nobody is bailed out.

The failure being engineered against is **silence**: a market's data source publishes
nothing, or nothing fresh, before its deadline. dClutch has **two** layers against that,
and only the first is built.

**Layer 1 — the disclosed failure outcome. LIVE, and witnessed on real ELFs.** Every
product carries one extra outcome beyond its ordinary ones
(`crates/dclutch-product-runtime-v2/src/lib.rs:202-211`). At founding the founder prepays
three escrow compartments. If the source is silent past
`window.end_unix_seconds + window.max_age_seconds`, **any stranger** sends
`CommitDeadlineFailure`, is paid the market's own pre-quoted bounty out of the failure
compartment, and the market settles onto the failure outcome everyone knew about when
they traded. Route census row `docs/reference/routes.md:203` reads
*executed (relayed-vertical); executed (resolution-relayed-programtest); refused
(resolution-relayed-programtest)*. The honest control is a non-ignored real-ELF test
named `a_silent_provider_cannot_strand_a_market_and_the_walker_is_paid`
(`crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs:3164`).

**Layer 2 — recovery. NOT built. This is the ruling.** A market could *additionally*
pre-buy up to four **named alternative sources**, in order, each with its own deadline and
its own prepaid bounty. `RecoveryAttemptV2` is exactly that:
`{source_spec_id, provider_release_id, deadline_unix_seconds, funding_allocation_id}`
(`crates/dclutch-source-contract/src/source_recovery_policy_v2.rs:26-30`), and
`provider_release_id` is what lets leg 0 be a different provider *family* — a genuinely
independent second opinion, not a retry.

> **So the ruling is: does dClutch want markets that can pre-buy named backup feeds, so a
> dead primary degrades to a second opinion instead of straight to "the market failed"?**

The money is always the founder's, always escrowed before the market opens, and always
pays a crank-turner. Never trader collateral, never Hoard principal.

### THE FAILURE MODE WITHOUT IT IS NOT STRANDING

This is the fact that reframes the ruling, and the contract does not carry it. **A market
whose only source goes silent has a live, tested, prepaid, permissionless exit today.**
Cutting recovery costs C-09 nothing: C-09's "disclosed fallback paths" is Layer 1.
Keeping recovery rescues nothing Layer 1 does not already rescue. What recovery buys is
**resolution quality** — a second source's answer instead of the failure outcome — not
liveness.

The one shape with no terminal is a market whose material *carries* a recovery policy:
`exhaust_after_primary_deadline` refuses it outright
(`crates/dclutch-source-contract/src/source_resolution_v2.rs:466-467`,
`RecoveryNotExhausted`) and no ladder exists. **Nothing is in that shape, because
`CreateFund` refuses to mint one** (`0x3011`, refusing site
`programs/dclutch-core-sbf/src/resolution.rs:719-726`), there is no mainnet
authorization, and devnet is disposable. I looked for a recoverable-stuck case before
concluding: `reopen_link_id` is decoded and read (`source_resolution_v2.rs:281,636`) and
**set by nothing**, and there is no fourth transition.

### Two claims in the contract's own row are stale — `docs/MASTER_COMPLETION_CONTRACT.md:187`

1. *"the per-leg `FailNext` walk sits inside `#[cfg(any())]`, so the ELF builds because
   the block never compiles."* **There is not one live `#[cfg(any())]` attribute in the
   tree.** All eight matches are prose in comments describing the class historically
   (`programs/dclutch-core-sbf/src/{lib.rs:150,resolution.rs:866,tests.rs:307}`,
   `programs/dclutch-resolution-proof-sbf/src/funded.rs:19,23,28`,
   `crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs:916`,
   `tools/local-validator/bootstrap/successor/src/market.rs:12831`). The blocks were
   deleted at `26b216d6`. The row's *conclusion* survives on a different and live premise
   — `exhaust_after_primary_deadline`'s refusal — but its stated mechanism is gone.
2. The `#[ignore]`d CU table is real (`crates/dclutch-svm-harness/README.md:82-83`) but
   is **retracted in bold in the same file** at `:93-94`. Disclosed-stale, not silently
   stale: cleanup, not a live falsehood.

`funded::process_funded_transition` **has no definition anywhere in the tree, in any
language** — verified tree-wide, zero hits. Its six occurrences are all comments, and
`tools/doc-citations/baseline.json:11-16` already lists them as known-broken citations.
`programs/dclutch-core-sbf/src/resolution.rs:878-882` remains a lifting plan whose first
step is *"resurrect"* a symbol nobody can locate. **A plan whose first step cannot be
performed reads as tractable work and is not.**

### Foundation or museum — **split, and the split is the useful answer**

**Foundation.** `RecoveryPolicyV2` is already read from a real Solana account on chain:
`programs/dclutch-core-sbf/src/resolution.rs:731-747` authenticates a finalized record of
exactly `RECOVERY_POLICY_BYTES_V2` under `RECOVERY_POLICY_SCHEMA_ID_V2` and decodes it.
It has a live record kind (`resolution-proof-sbf/src/lib.rs:408`), a Lean-owned ABI, and
a 13-case refusal corpus. **The persisted state account already admits the phase**:
`SourceResolutionStateV2::validate_shape` has a full `Recovery` arm
(`source_resolution_v2.rs:668-676`), `active_attempt` has its own ABI offset (`:321`),
and `Resolved` already accepts terminal route `Recovery` (`:683-684`). *Only the
transition function is missing — no account format change, no ABI change, no migration.*
The three-compartment escrow is already pinned on chain for recovery material
(`resolution.rs:779-801`).

**Museum.** `RecoveryPolicyV1` / `RecoveryAttemptV1` / `RecoveryMaterialSlotV1` /
`fail_next_view` / `exhaust_view` — 382 lines in `source-contract/src/lib.rs`, keyed to
`product_instance_id`, an identity the live V2 entrance never emits. `AGENTS.md:284-285`
requires deleting these **whichever way the ruling goes**: a "keep" ruling replaces the
V1 half, it does not preserve it.

### Cost basis, from the tree's own landed routes

Recovery-specific surface today: **1,770 lines** across six named spans, plus ~350 lines
of scattered tests. **35 tests** (10 fixture, 16 component, 9 real-ELF, 0 on-chain),
**1 `#[ignore]`d**. Zero non-test callers of any constructor. The load-bearing caveat: all
8 non-ignored real-ELF tests build a `RecoveryPolicyV2` and then found their material with
recovery `None` — **no test anywhere executes a recovery transition on a compiled ELF.**

| precedent | commits | files | +lines |
| --- | --- | --- | --- |
| `CommitDeadlineFailure` walk (closest analogue) | `92b137d1`…`87e45906` (8) | 46 | 12,552 |
| — minus its new wire crate | 6 | 30 | 5,614 |
| `sponsored_push_v1` (tightest, one commit) | `bb405b12` | 17 | 7,639 |
| pre-market funding abort leg | 4 commits | 26 | 5,319 |
| **the weld that cut it (`12d0deb5`)** | 1 | 8 | **251** (code-only: 4 files, +211, of which 134 are tests) |

**Building ≈ 5,000 inserted lines across 11–30 files and 4–6 crates. Cutting is ~4% of
building.** Four named pieces: write `process_funded_transition`; add a fourth
`fail_next`-shaped transition; un-weld `recovery_walk_has_a_live_route`
(`resolution.rs:884`) plus its operator mirror
(`crates/dclutch-resolution-core-v3-operator/src/lib.rs:1054,1132`) plus the hostile at
`core-sbf/src/tests.rs:355` designed to go red that day; **and an authoring
constructor** — `docs/OMISSION_INDEX.md:88` records that the policy is keyed to an
identity the live entrance does not emit, so the ladder cannot currently be authored for
any product at all. That last piece is additional to every precedent above.

### Who pays

Founder's external wallet → System transfer, **before Core creates the Market**
(`programs/dclutch-resolution-proof-sbf/src/pre_market_funding_v1.rs:69,166-170`) →
`FundingLedgerV2` PDA → `FundingCompartment::Bounty`
(`crates/dclutch-capability-contract/src/funding.rs:301`) → the stranger who cranks it.
`release_in_place` has **exactly one non-test production call site tree-wide**
(`programs/dclutch-resolution-proof-sbf/src/funded.rs:212`), guarded at `:181-186` to the
failure entry only; the recovery and exhaustion rows sit prepaid until `CloseFund` refunds
them. Hoard principal is separable **by derivation, not by a check**:
`CompartmentV1::HoardPrincipal = 3` is a PDA seed byte
(`crates/dclutch-custody-contract/src/lib.rs:212,406-415`), so the Hoard vault and any
bounty vault are different addresses.

### The framing for ember, in one line

`AGENTS.md:69-71` already states the project's prior: *"Optional venues, liquidity,
wrappers, bearer mints, and **recovery depth** are capability children, not universal
ontology."* Recovery as a *capability child* — sold to the markets that want it, absent
from the ones that do not — is what the V2 machinery is already shaped for. The question
is whether any market wants it enough to pay ~5,000 lines plus an authoring path, given
that no market can be stranded without it.

---

## A3. Provider-family scope — **EMBER'S on breadth; the refactor is DECIDED and its stated reason is already fixed**

### The economics, verified — and the target moved

**Switchboard's current Solana path is not what a dClutch family would have been built
against.** As of 2026 the recommended integration is **Oracle Quotes** via the quote
program (`queue.fetchManagedUpdateIxs(...)`), and Switchboard's own documentation says
the classic On-Demand `PullFeed.fetchUpdateIx(...)` path is *"legacy compatibility only"*
(docs.switchboard.xyz, fetched 2026-09-01). So there are three generations, not two: V2
push (legacy, surviving mainly as a data *source* inside aggregator feed definitions),
On-Demand PullFeed (explicitly legacy), and Oracle Quotes / Surge (current).

**The current shape is structurally incompatible with the profile dClutch already has.**
An Oracle Quote is consumed by running three instructions in one transaction —
Ed25519 sigverify → quote program `verified_update` → consumer read. Nothing verifiable
persists across transactions. Pyth's pull path instead persists an already-verified
`PriceUpdateV2` account whose `verification_level` a later transaction can read. **dClutch's
sponsored-push profile exists precisely to capture evidence before a deadline and consume
it after** — a same-transaction quote cannot serve that.

**Permissionless: yes for creation, but the consumer pins a queue.** Feed and queue
creation are open. What a consuming program actually verifies is that the quote account is
the canonical PDA derived from **a queue pubkey the consumer names**, plus an Ed25519
sigverify instruction earlier in the same transaction, plus staleness. The trust anchor is
the named queue and its TEE-attested, SWTCH-staked, slashable oracle set — not an open set.
There is also an `authority`-updated variant Switchboard warns about explicitly
(*"Consumers should trust them only to the extent that they trust the configured
authority"*), which a dClutch family would have to refuse by name.

**Cost per update: still unverified, and that is itself the finding.** Switchboard's own
documentation corpus — sitemap, `llms-full.txt`, FAQ, protocol page — contains **no SOL or
lamport figure for an oracle update anywhere.** The only number found is secondhand
(20,000–100,000 lamports/update; rareskills.io, 2026-02-25) and untraceable to Switchboard.
Published instead: Surge subscription tiers (Plug free / Pro ~$3,000 mo / Enterprise
~$7,500 mo, **paid in SWTCH**) and read cost ~2,000 CU Anchor, ~190 CU Pinocchio. Quote
accounts are created lazily on first update, payer = the transaction payer, i.e. the
consumer at read time. **After a direct search of the primary source, the economics on
record as "secondhand and unverified" remain secondhand and unverified.**

**Pyth, apples to apples.** Mechanism verified in program source:
`pyth-solana-receiver/src/lib.rs` charges `config.single_update_fee_in_lamports` on every
`post_update` (`pay_single_update_fee()`, lines 459/490/598-629), guardian signatures via
`secp256k1_recover`. The fee *value* could not be read on chain — public RPCs refuse
`getProgramAccounts` — so the widely-repeated 1 lamport is secondhand too. Rent computed,
not quoted: `PriceUpdateV2::LEN = 134`, so `128 + 134` bytes ≈ **1,823,520 lamports
(0.00182 SOL), fully reclaimable** (`closeUpdateAccounts: true`). Permissionlessness is
documented: *"updating a price feed is a permissionless operation, and anyone can run this
process."*

**Does a third family buy anything real?** On assets, mostly no — and worse than no.
Switchboard's aggregator config sources *from Pyth* (`pythAddress`, `pythPushFeedId`) and
from Chainlink, so a Switchboard feed on a major asset is frequently a wrapper around a
Pyth price, which makes it a poor independence hedge for exactly the assets a prediction
market cares about. Pyth carries first-party feeds Switchboard has no equivalent for
(US and HK equities, metals, crude, major FX; 3,501 live instrument feeds). Switchboard's
real differentiator is **arbitrary non-financial data** — any HTTP endpoint, off-chain
computation, other contracts' state. So:

> **If C-09's breadth objective is asset breadth, Switchboard adds nothing Pyth lacks and
> adds a dependency that often re-reads Pyth. If the objective is non-price resolution
> sources, Switchboard is a real addition — but that is a different question than "which
> oracle", and it is the question worth putting to ember.**

### Two corrections to the in-tree premises

**The closed set has four variants, not three.** `SourceAccessProfile`,
`crates/dclutch-source-contract/src/lib.rs:824-846`: `PythTerminalOneTransaction = 1`,
`SharedObservationChild = 2`, `RelayedObservationRecord = 3`,
`PythSponsoredPushSnapshot = 4`. The ruling ember quoted is at **`:294-300`** at this
HEAD, not `:268-283`. `lib.rs:869-882` reinforces it: the access profile *"is the
extension discriminator, and it is the only one."*

**"5/5, 1/1, 19/19" are exactly test counts, and one of them is one test.** Verified by
counting `#[test]`/`#[tokio::test]`: `resolution_core_v3_lifecycle.rs` 5,
`sponsored_push_lifecycle.rs` **1**, `relayed_mainnet_state.rs` 19; zero ignored.
"1/1" is one integration test for an entire profile.

### The cost precedents are not cost bases — **REFUTED**

Neither the ~4,940 nor the ~12,900 has a commit behind it. Both were introduced at
`e1a52b63` with no shas, and both reconstruct **exactly as `wc -l` file censuses at HEAD**
(4,941 and 12,900 respectively), not as diffs. The real landing commits:

| | commit(s) | files | +ins | −del |
| --- | --- | --- | --- | --- |
| **profile** (sponsored push) | `bb405b12` | 17 | 7,639 | 31 |
| profile, full arc | + `c7e3f617`, `98adf2b3`, `9db5ff17` | 25 | 8,782 | 112 |
| **family** (relayed) | `92b137d1` + 7 siblings | 53 | 12,455 | 480 |
| family + its consumer | + `983a9122`, `d3f1c241` | | 14,939 | |

**The profile figure understates by 55%** — the 2,428 lines of operator tooling without
which the profile is not exercisable, plus docs and a TS mirror, are excluded. The family
figure is coincidentally close by a different measurement and hides that 23% of it is
Lean-emitted from 1,820 lines of Lean the census does not count. Like for like a profile
is **59–60%** of a family, not 38%. **`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:1820`'s "~13,000 lines for Switchboard"
inherits every exclusion and should be read as a floor; 15,000–17,000 is the honest
number on the relayed precedent — and that is for the *wrong shape*, per the target move
above.**

### The generic-header refactor — **DECIDED, and its headline justification is stale**

`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:1826-1828` says `provider_v3.rs:372` pins the provider-neutral
`transport_profile_id` to a Wormhole router ABI id. **At HEAD it does not.**
`programs/dclutch-resolution-proof-sbf/src/provider_v3.rs:372` reads
`source_release.transport_profile_id().to_bytes() != pyth.transport_profile_id()` —
neutral on both sides. `01478abe` (2026-09-01 09:52, an ancestor of HEAD) fixed it:
*"resolution: transport_profile_id stops meaning router_abi_id."* The family-specific
answer moved one level down to `crates/dclutch-pyth-svm/src/release.rs:344-346`, with the
argument written out at `:319-343`: *"A provider with no router is not thereby excluded
from the protocol: it supplies its own transport identity through its own release type,
and the neutral record never learns what a router is."*

**The premise "a provider without a router cannot satisfy the neutral record" is refuted
by the tree itself** — the sponsored-push family already does exactly that
(`sponsored_push.rs:145,404,499`). One hand-written dispatch survives at
`tools/local-validator/bootstrap/successor/src/market.rs:12099-12103` and is now
redundant, since both arms return the same value. A few lines.

**The surviving argument is stronger than the one that was retired.** `PythReleaseV1` is
440 bytes / 18 fields; `PythSponsoredPushReleaseV1` is 592 bytes / 22 fields, with **zero
overlap in transport fields** — the sponsored release drops the whole router+guardian
block and adds a push-oracle block. Two members of the *same family* disagree on layout,
which argues the decomposition from inside with no second family needed. The "9 generic
to 9 Pyth-shaped" split is defensible but not forced — `price_update_codec_id` and
`upstream_commit` are generic concepts with Pyth values. **The number that is forced is
6**: `router_program`, `router_programdata`, `router_abi_id`, `router_deployment_slot`,
`guardian_set_count`, `required_guardian_count` are the only fields a router-less family
literally cannot fill.

**Refactor cost on its own: 300–600 lines across ~6–10 files, and no identity moves.**
The neutral `ProviderReleaseV1` is already generic (five `ContentId`s, 176 bytes, magic
`DCLTPRV1`, `lib.rs:115,190,469-475`) and is embedded at fixed offsets inside larger
records — so changing *it* would move digests, but decomposing the family-side
`PythReleaseV1` behind that seam does not. `PythReleaseV1` has no TS or Lean mirror.
Roughly 4–7% of a new profile. **This is engineering, already authorized, and it does not
wait on the breadth ruling.**

---

## A4. Claims split/merge — **the coordinator's decision survives; see §A4b for the outer-route cost**

One correction to the framing before the evidence: **"an orphan with not even a Cargo
dependency" is stale.** `crates/dclutch-operator/Cargo.toml:31` now depends on
`dclutch-claims-conservation-contract`, and `crates/dclutch-operator/src/claims_conservation_v1.rs`
uses it — the `1d1c2453` work GOAL.md describes as *"the first `DCLCNS01` constructor and
the first Cargo dependency that contract has ever had."* The decision is already landing.

Full cost basis in §A4b below.

---

## B1. Materialize / Dematerialize → delete — see §B1b below

## B2. The two `dclutch` binaries → **the decision is BACKWARDS. DECIDED: rename the TypeScript one.**

The coordinator decided to rename the **Rust** binary. That is the only one anyone can
install.

**The TypeScript binary is not published and has no users outside this repo.** Five
independent negatives: `packages/dclutch-cli/package.json:4` is `private: true` (npm
publish refuses outright); `@dclutch/cli` and `@dclutch/sdk` both **404** on npm and the
scope search returns `{"objects":[],"total":0}`; no `.npmrc` anywhere; no `npm publish`
or `NPM_TOKEN` in any workflow; and the tree says so itself at
`docs/guides/client-developers.md:21` — *"Neither is on npm yet."*
`packages/dclutch-cli/.gitignore:10` ignores `/dist`, so the bundle is never committed
either.

**The Rust binary is the released, user-facing artifact.** `tools/dclutch-cli/Cargo.toml`
carries `[[bin]] name = "dclutch"` (`:20-21`), `publish = false` (`:10`) with the comment
at `:24-27` saying plainly *"`publish = false` keeps this off crates.io… It is still the
distributed artifact,"* and `[package.metadata.dist]` (`:28-29`). It ships as signed
cargo-dist release tarballs for three targets plus a shell installer, on GitHub, tags
`v0.1.0-devnet.1/.2/.3`, **with nonzero download counts** (installer 5; aarch64-darwin 3
on devnet.2 and 2 on devnet.3; x86_64 darwin/linux 1 each per release). `dist-workspace.toml`
on the host's `origin/main` names exactly `["cargo:dclutch/tools/dclutch-cli"]` and nothing
else.

**The inversion is the hazard.** Three release runners actually execute the TS binary
(`tools/release/checked-release-candidate.sh:740-761`,
`tools/release/public_route_campaign.py:48-51,573-601`,
`tools/release/successor_campaign_pack.py:623,711`) and reference it **by path**, not by
PATH lookup — so renaming its bin is safe. No script in the tree invokes the Rust binary
at all; every reference to it is prose or a cross-reference. **The binary with all the
in-tree machinery is the unpublished one, and the binary with external users has none.**

**Where the wrong belief came from, and it is load-bearing.** Both binaries' own source
comments assert the npm publication that does not exist:
`packages/dclutch-cli/src/main.ts:180` — *"This one is `@dclutch/cli`
(`packages/dclutch-cli`, npm)"* — and `tools/dclutch-cli/src/main.rs:141` — *"installed by
npm as `@dclutch/cli`."* Both false against `private: true` and a 404 registry, and both
sit in the exact two files anyone would open to check. **Repair these before anyone
renames anything**; they are the citation-outlives-the-thing shape from C-16 §4b, one
level out.

**The overlap claims: three verified, one over-stated.** `--keypair` refused by name in
the Rust CLI — verified, and I looked for an admission before concluding: across all
tracked Rust sources `--keypair` appears **only** in refusal paths
(`tools/dclutch-cli/tests/ticket_v1.rs:237-256` spawns the real binary and asserts
`!status.success()` with `"{flag} was accepted"` as the failure message;
`src/general.rs:190-196`; `src/ticket.rs:481`; `src/fractional.rs:797`), and the only
admitted form is `--keypair-env` (`ticket.rs:435`, `main.rs:219`); the TS CLI accepts it
normally (`packages/dclutch-cli/src/main.ts:100`, `src/context.ts:208-223`). Env vars:
`DCLUTCH_RPC_URL` (`tools/dclutch-cli/src/main.rs:96`) vs `DCLUTCH_RPC`
(`packages/dclutch-cli/src/context.ts:124`). `market` (`main.rs:127`) vs `markets`
(`main.ts:240`). **But "they overlap only on `help`" is wrong**: each binary knowingly
*names the other's verbs* and refuses them with a sentence pointing at the other program
(`main.ts:194-209`; `main.rs:140-141,180`, gated by tests at `main.rs:453` and
`packages/dclutch-cli/test/deployment.test.ts:195-208`). The disambiguation exists in
`--help`; it is the *docs* that lack it. `docs/` teaching no TypeScript build — **verified
exactly**: the only two `docs/` hits for build commands are about the web suite, and the
sole build instructions live at `packages/dclutch-cli/README.md:8` and inside the
launcher's own error string (`bin/dclutch.mjs:8`).

---

## B3. The heap — **DISSOLVED to a measurement. The ruling's premise is a comment describing deleted code.**

The ruling was: keep the 64 KiB ceiling and stop claiming `admitted`, *because* capping
the scratch at the 32,768 default would make the extended heap useless to the routes that
need it, `direct_hot_top_level` among them.

**`DCLTHOT3` was put on the extended-heap list for a reason that was deleted the same
day, by a descendant commit, and never re-measured.**

- `8ee544e4` (2026-08-30 13:00) added `DCLTHOT3` to `declares_extended_heap_profile_v1`.
  Its justification survives verbatim in source at
  `programs/dclutch-trading-sbf/src/entrypoint_adapter.rs:1298-1308`: *"a caller who
  invokes Trading DIRECTLY … makes two Registry reauthentication CPIs that a Registry
  continuation never makes, and holds their frames and receipts against an allocator that
  never frees."*
- `f04654a0` (2026-08-30 23:16, a descendant) landed decision 0017 option B: the top-level
  arm reads the activation cache instead of invoking the Registry.
- At HEAD, `reauthenticate_top_level_root_roles_v3`
  (`programs/dclutch-trading-sbf/src/hot_v3.rs:4898-4915`) makes **no CPI**: heap check,
  `require_cache_account`, then a local borrow-and-decode. **Its own doc, forty lines
  above the stale one, says so** (`hot_v3.rs:4880-4882`): *"invoking
  `RegistryInstructionV1::Reauthenticate` was LEGAL on this arm — and it is what this
  route did until decision 0017's option B. **It stopped.**"*

Two comments in the same program contradict each other, and the ruling was taken on the
stale one.

Supporting arithmetic: the continuation route, which never made those CPIs, peaks at
**29,895 of 32,768 bytes — 2,873 spare** (`docs/evidence/ASPIRATION_LEDGER_2026_08_27.md:1068-1070`); CPI
instruction clones on this allocator measure **2,322 bytes for two child CPIs**
(`entrypoint_adapter.rs:1394-1396`). The top-level route at HEAD now differs from the
continuation by *less* than when the declaration was written.

**The list is six routes, and only one has a measured need.**
`declares_extended_heap_profile_v1` (`entrypoint_adapter.rs:1294`) admits `DCLTHOT3`
(justification stale, above), `DCLTSEL1` (`:1330` says outright *"The seal outer's own
peak is NOT measured here"*), `DCLTGMF3`/`DCLTGFP1` (module-doc reasoning, no
measurement), **`DCLTPCB2` — the one real one**, a measured OOM whose peak is *"the sum"*
of three stages held live (`:1289-1293`), and controller-funding-prepare (no reason
found). Only `DCLTHOT3` and `DCLTSEL1` have a gate at all; the founding four rely on
`lift_declared_heap_profile_v1`, documented as *"best effort by construction"* — absent
grant, they allocate until they die.

**The scratch is sized for a worst case no executable route reaches.** Only two
`HeapScratchRegionV1`s exist (`hot_v3.rs:3803`, `:6050`), bumping **down from
`bytes_capacity()`** — which is the whole mechanism of the fault. Bank sizes are functions
of runtime-account count and of K via `tail_count` (`hot_v3.rs:3914-3919`); at the declared
caps (256 accounts, 512 scalars, 128 identities, `hot_v3.rs:431-434`) eight live banks
reach ~32,768 bytes of registers on their own, which is what makes 64 KiB look necessary.
The canonical Direct demo is three outcomes and the journey's market four
(`program-test/direct-hot/src/waist.rs:705-706`), and the measured whole-heap peak on the
canonical bundle is 29,895.

**Kernel boundary: this is correctly an adapter need**, not a kernel violation — every
consumer is in `entrypoint_adapter.rs` or `hot_v3.rs`, and `hot_v3.rs:6118-6120` says so:
*"The kernel allocates nothing, so the runtime-write overlap refusal's scratch is one of
this function's banks."* It is a deliberate arena.

**So the ruling narrows to the founding routes.** If `DCLTHOT3`'s peak now fits 32,768,
capping the scratch costs the trade path nothing, and the founding routes are untouched
by the cap because they use the *upward* bump (ordinary `Box`/`Vec`), not the scratch.
**Cost to settle: one run of `direct_hot_top_level` under `--features hot-cu-profile`,
reading the `dclutch-hot-heap:` marks** — the instrument already exists
(`hot_v3.rs:588,630-634`, 20+ `hot_heap_mark!` sites). Caveat: that feature lifts every
route into the declaration list (`entrypoint_adapter.rs:1295`), so profiled CU totals are
not shipped totals — fine for reading a peak.

**And the abort path throws away exactly the machinery the refusal path has.** An access
violation carries no `Custom` code, so all three arms of `readReportedRefusal`
(`apps/dclutch-web/lib/explorer/refusals.ts:164,202`) miss and it falls through to
`runtimeErrorLabel` (`:234`), which returns the discriminant verbatim: the user sees
**`InstructionError #3: ProgramFailedToComplete`** — no heap, no budget, no remedy — while
`Access violation … at 0x30000fcf8` sits unparsed in `logMessages`
(`transaction.ts:204`). Meanwhile `0x4008 TradingSbfError::HeapFrame` is already fully
served with a meaning string in both generated registries
(`apps/dclutch-web/lib/generated/refusalRegistryV1.ts:119`,
`packages/dclutch-sdk/lib/generated/refusalRegistryV1.ts:119`). **That asymmetry is the
real cost of choosing "abort" and it is not in the ruling's statement.**

**Two incidental finds worth routing.** (1) A fourth instance of the accused shape, in a
second program: `programs/dclutch-general-accelerator-sbf/src/lib.rs:440-459` scans for a
`request_heap_frame`, sets a variable named `heap_granted`, and refuses
`HeapFrameNotGranted` — the value is the *request*. Harmless there (that program cannot
use an extended heap), but the same name-promises-what-it-cannot-observe defect.
(2) `entrypoint_adapter.rs:287` declares `HEAP_HEADER_BYTES = 24` while `docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:3632-3634`
explains the 776-byte fault offset as *"the bump heap reserves 776 bytes at its floor"* —
those disagree by 752, and since the scratch bumps *down from the ceiling*, a fault at
`ceiling − 776` is more simply the first scratch block being 776 bytes wide. The
observation (fault tracks the request) is unaffected; the attribution looks wrong.

**The "six campaigns call `set_compute_max_units`" figure does not reproduce.** 33 distinct
`.rs` files call it at HEAD, and no scoping yields six; the claim was never enumerated
anywhere in the tree. What *is* exactly verifiable is the intersection that matters — files
that force the budget **and** put a `RequestHeapFrame` on the wire — and it is two
(`general-accelerator/…/freeze.rs:220,397`, `lifecycle.rs:1120,1307`), both **inert**,
because those fixtures load only the accelerator, which uses the stock
`solana_program::entrypoint!` whose BumpAllocator never reads the request. Every Trading
extended-heap campaign is on a non-forced budget. The conclusion of the earlier
investigation stands; its count does not.

---

## B5 + B6. The 9000 ceiling and zero-cut markets — **BOTH coordinator readings are wrong, in opposite directions**

### `MAX_CELL_EX_ANTE_SHARE_BPS_V1 = 9000` — the "ceiling on the ceiling" reading is **REFUTED**

The constant is at `crates/dclutch-product-compiler/src/partition_quality.rs:72` and has
**zero production readers** — every other occurrence is a `#[cfg(test)]` block or a
`pub use` re-export. Its own docstring (`:69-71`) already says it: *"callers state their
own, because the ceiling is a product decision and this constant is only the one the
entrance defaults to naming."* So far the coordinator's reading holds.

**But it does not cap the author.** The comparison is
`report.is_degenerate(ceiling_bps)` where `ceiling_bps` is the author's number passed
verbatim, and the only bound anywhere is `1..=10_000` — at the compiler
(`partition_quality.rs:260`) and at both author-side validators
(`tools/local-validator/bootstrap/successor/src/spline_product.rs:487`,
`market.rs:3119`). **An author may state 10000, which admits every partition except an
exactly-100% cell.** The constant is neither a default nor a cap; it bounds nothing.

**Also: the coordinator looked at the wrong type.** `FoundingBandV1`
(`partition_quality.rs:80-91`) is `{anchor, denominator, volatility_bps, window_slots}` —
**it has no `max_cell_share_bps` field at all.** The field lives on
`FoundingBandInputV1`, of which there are two independent copies
(`successor/src/model.rs:83-95` and `successor/src/spline_product.rs:87-99`), required in
both, carried as `MarketRunInput.founding_band: Option<…>` (`model.rs:115`).

**And no authoring surface varies it.** Every non-test caller writes `9000`:
`successor/src/main.rs:763,817`, `local_mutable.rs:1495,1652,1686`,
`docs/operator/examples/spline-product-degree2.json:27`, and the hardcoded
`market.rs:12273`. **`apps/dclutch-web` has zero occurrences** of `max_cell_share_bps`,
`maxCellShareBps`, `founding_band`, or volatility-as-input — the wizard cannot call this
gate at all; it runs a strictly weaker TS unit-sanity check with its own provisional
constant that says so (`apps/dclutch-web/lib/founding/rangeProtection.ts:207-234`).

**So what remains ember's is three things, not zero:** (i) the number 9000, which is de
facto the policy because every real caller types it; (ii) whether an author may state a
ceiling above 9000 at all, which today they may, up to a gate-disabling 10000, and nothing
in the tree defends that; (iii) whether the web wizard should author a band at all, given
that markets founded through it are never measured by the real gate.

### Zero-cut markets — **product reading CONFIRMED, mechanism wrong, and there is a live break**

The coordinator is right that a relayed observation record is a different product shape,
**but not for the stated reason.** `tools/local-validator/bootstrap/successor/src/relayed.rs:385`
declares `cuts = []` with `coefficients = [1, 0]` and `outcome_count = 2`, which is **one
cell, not zero** (`outcome_count = cuts.len() + 2`, `market.rs:12288`; the measured
partition is `cuts.len() + 1`). And it **mints heavily** — `relayed.rs:536`
`initial_collateral_atoms: 1_000_000_000`, `:585-598` admits 500,000,000 atoms of founding
principal against `claim-basis/unit-complete-set`. So "it mints nothing, therefore it is a
different shape" will not survive contact.

The correct reason: holders do not all get the same payoff. `coefficients = [1, 0]` — the
ordinary region pays 1 and the failure region pays 0 — and **the discriminating outcome is
the failure region, which is not a partition cell**: `assess_partition_quality_v1` measures
ordinary cells only (`partition_quality.rs:205`). The tree admits the tension in its own
words at `model.rs:104-107`: *"`relayed_market_input` compiles a market with NO CUTS,
which the compiler's own documentation calls legal and which is degenerate by construction
under any quality model."*

**"A zero-cut market stays ungated" is REFUTED — it is not ungated, it is bricked.**
`relayed.rs:532` sets `founding_band: None` with a deliberate comment (*"it declares no
belief rather than fabricating one it would never be measured against"*), and the founding
path `campaign.rs:4575 → market.rs:1954 publish_market_records → market.rs:3465
compile_market_bodies` hits an **unconditional** requirement at `market.rs:3172`:
`input.founding_band.as_ref().ok_or_else(…)` — *"founding_band is required to compile this
market's partition… There is no default."* No zero-cut branch. So founding the relayed
graduation market today refuses **"founding_band is required"**, not
`DegenerateOutcomePartition`. And that hand-off is the release story's own: the caller
target is recorded as `"dclutch-local-successor-bootstrap/graduation-market +
campaign/found"` at `tools/devnet-scenarios/fixtures/graduation.json:675` and
`tools/devnet-scenarios/src/engine.rs:466`. Nothing goes red because the SVM harness
(`crates/dclutch-svm-harness/tests/relayed_mainnet_state.rs:670-700`) builds the zero-cut
product through `compile_result_domain_v2`/`compile_portfolio_v2` directly and never
touches `compile_market_bodies`.

**And the second path is the bigger question.** Zero cuts is **the protocol's own
two-outcome floor** — `programs/dclutch-trading-sbf/program-test/tests/registry_hot_continuation.rs:1090`:
*"Every market from two outcomes (zero cuts, the protocol floor) to ten trades on the same
descriptor."* `market.rs:12283-12290` documents a `LocalMarketShapeV1` with empty cuts as
*"legal and the narrowest market this compiler can emit."* That path **is** gated now, and
one ordinary cell always scores 10000 bps, so **a legal width-2 market refuses
`DegenerateOutcomePartition` whenever it is founded through `compile_market_bodies`.** The
documented capability at `market.rs:12283` is dead at HEAD.

> **The gate's coverage of legal geometry has a hole at width 2, and that — not relayed —
> is what needs deciding.** It also stacks with B4: if K = 2 is the executable width and
> width 2 cannot pass the partition gate, two rows are in tension.

---

## C1. The mixed-unit solvency gate — **DECIDED. Scale already has one semantic owner, and it is neither candidate.**

**The defect, re-verified red at HEAD.** `residual_at`
(`programs/dclutch-trading-sbf/src/dealer/v3_equity.rs:420-431`) computes
`collateral + claims[s] − obligations[s]` in one `u64` and that scalar is the sole
`Insolvent` verdict (`:280`, `:377`). Run filtered with `--test-threads=1`:

```
the_solvency_gate_must_not_change_its_verdict_with_the_claim_unit ... FAILED
assertion failed: the same 20 atoms of claim value must give one solvency verdict,
but the gate reads the unit count: Ok(PoolEquityPlanV3 { … }) vs Err(Insolvent)
```

`basis_scale` occurs **three times in the entire Dealer stack**, all three inside that
test's own doc comment (`v3_equity.rs:457,460,478`).

### Where scale should live, generally

**It already lives somewhere, and the tree already enforces it once.** The owner is
`ProductBasisV3::payout_scale` — atoms per complete set. Core authenticates it at
founding: `programs/dclutch-core-sbf/src/generic_founding_v1.rs:1091` refuses
`CoreSbfError::Reference` unless `product.payout_scale != 0`, `:1099` sets
`GenericProductFacts.basis_scale = product.payout_scale`, and `:1300` refuses `Reference`
unless `product.basis_scale == request.basis_scale()`.
`crates/dclutch-claims-conservation-contract/src/lib.rs:60-61` names it outright:
*"The conversion to atoms is `basis_scale`, and `basis_scale` is the authenticated
`ProductBasisV3::payout_scale` that Core pins into the founding intent."*

**So the general rule is: scale is a property of the payoff basis, and every consumer must
obtain it by authenticating the basis record against the market identity — never by
declaring it in its own descriptor.**

**The coordinator's proposed source is directionally right and names the wrong account.**
"The authenticated Core market state" cannot supply it: `CoreState`
(`crates/dclutch-market-core-codec/src/generated.rs:402-414`) is
`{phase, readiness, terminal_winner, identity, outstanding_capabilities,
principal_cap_sets, rent_beneficiary, terminal_receipt, bumps}` — **no scale field.**
Adding one would make Core a second author for a fact the product basis owns, which
`AGENTS.md`'s "one semantic owner per persisted fact" forbids. What `CoreState` carries is
the **pointer**: `MarketIdentity.product_record`.

And the descriptor is equally wrong for a reason worth stating: `DealerConfigV4`'s own
module doc (`config_v4.rs:1-9`) explains that the config is a *release artifact selected
by the manifest*, deliberately excluding Market-dependent facts — *"This record owns only
facts that do not depend on the Market address."* Scale is per-market. A `basis_scale`
field in `DealerConfigV4` would be an unauthenticated restatement of the basis's value,
with nothing checking the two agree.

### The repair is short, because the Dealer already loads the account it needs

`authenticate_core_market_v4` (`programs/dclutch-trading-sbf/src/dealer/v3_accelerator_accounts.rs:441-467`)
**already decodes `CoreState` from the real Core Market account and already pins
`core.identity.product_record`.** Its own doc comment at `:438-441` even names the gap:
*"the aggregate's `basis_id` has no Core-side field at all — it is authenticated against
the Product runtime's `semantic_basis_id` by the caller."*

So the route is: take the Product record as an additional account, authenticate it against
`core.identity.product_record` exactly as `generic_founding_v1.rs:1085-1100` does, read
`payout_scale`, and carry it into `PoolEquityInputV3` — weighing the claim leg by it.
That also touches `v3_composer.rs:323-335`, where complete-SET counts are handed to Custody
as transfer ATOM amounts, which is the same bug in the other direction and the test's own
docstring already names it.

**What I would have needed to see to decide otherwise:** a scale field on `CoreState`, or
a Dealer config whose digest is per-market in a way that could carry one without a second
author. Neither exists.

**One thing this does not fix, and it is bigger.** The conservation contract's own header
(`lib.rs:77-110`) records that `Σ_k claims_payout[k] == basis_scale` — the premise making
`quantity * basis_scale` the right deposit — is **enforced for the product vector and not
for the translated width-K vector**, and that on the generic settlement route the exposure
is not pinned to the Market at all: the one identity check that looks like it would catch
a substitution compares the instruction to itself (`exposure.rs:274` assigns `bundle_id`
from `admission.selected_id`, which `terminal_settlement_v3.rs:393-401` sets to
`input.exposure_id`). Repairing the Dealer's scale does not touch that. It is named here
so the two are not confused.

---

## C2. The empty-satisfying-set constraint — **DECIDED: repair in execution order, and the coefficient guard is LAST**

The constraint is real and the conviction is sound:
`crates/dclutch-bearer-v2-operator/src/open_structured_v3.rs:926-929` emits, per row,
`scalar_eq(coefficient[row], denominator)`, forcing `coefficient[i] == D`; the composition
kernel requires `gcd(D, coefficients…) == 1` (`translation.rs:231`) because the
coefficients *are* the numerators; together `D == 1`; but `D <= 1` refuses
`NonFractionalDenominator`, so `D >= 2`. Empty.

**The order is not a matter of taste, because WAVE already recorded the dependency:**
`Content`/`Route` is *"the live frontier, **and upstream of the coefficient question**"*
(`docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:4737`). A guard you cannot reach cannot be shown repaired. Combined with the
asymmetry the lane correctly refused to ignore — deleting a never-*refusing* tautology is
free, deleting a never-*admitting* guard **lets through what was blocked** — the order
follows:

1. **Walls upstream of it, in execution order, each with its own control.** Wall 3 has
   moved since that WAVE entry: `validate_account_profile_join_for_action` now exists
   (`crates/dclutch-account-profile-contract/src/lifecycle_v3.rs:935,1386`) and the
   runtime calls it (`programs/dclutch-trading-sbf/src/hot_v3/seal.rs:642`), with the
   reason written in the file (`:629-640`). Wall 4 is whatever stands behind it, still
   unknown because nothing has executed past it.
2. **Only then the coefficient guard**, removed at the moment when its removal is the sole
   remaining difference between a red route and a green one — because that is the only
   configuration in which its removal has an honest positive control. Removing it earlier
   produces a route that is still red for a different reason, which proves nothing and
   makes the family *less* checked in the interim.

The intermediate step already used — keep the constraint's shape and make its second
operand unfailable — is a **probe, not a repair**, and it is exactly the right instrument:
it revealed wall 3 without weakening anything that ships.

> **The general form worth writing down: a never-admitting guard may only be deleted when
> the route it blocks can be shown to execute in the same change. Until then it is a
> wall's tombstone, not a defect to be tidied.**

---

## C3. What C-16 completion actually means — **DECIDED, and the answer is already written; what is missing is one correction**

`docs/evidence/C16_ENTRY_LIST_2026_09_01.md` already makes C-16 operational and does it
well. C-16 permits *independent hostile review to begin* — not completion — when six
categories are clear, and the entry list gives each an instrument and a number:

| § | category | at measurement |
| --- | --- | --- |
| 1 | never-executed intended route | **55 of 161** (73 witnessed, 33 blocked-with-reason); 2 instrument rows closed |
| 2 | user-inaccessible capability | **19 selectors**; the honest C-12 route number is **20** |
| 3 | stale claim | **6 unfixed** of 8 verified |
| 4 | material gap | 1 dead refusal code, 11 orphan items, 4 stale-open `OMISSION_INDEX` rows |
| 4b | prose citing deleted symbols | 5 comments across 4 files, instrumented at `tools/doc-citations/` |
| 5 | unexplained authority | 3 findings, 2 benign, **1 open hazard** |
| 6 | unowned economic flow | atom half swept (103 sites), **lamport half open** |

So "C-16 completion" already has a precise meaning: **every one of those numbers is zero
or carries a dated ember ruling.** The largest block, 55 routes, *"is not adjudicable by
argument — only by campaigns."* Nothing about the definition is open.

**The one thing that is wrong is the ruling table, and it is wrong in exactly the category
the document instruments.** `C16_ENTRY_LIST_2026_09_01.md:417` lists **R-8 (C-15,
FHE/MPC) as open**. It was written at `3466740e` (12:02) and ember ruled C-15 **out of the
accepted project** at `5a371810` (12:47) the same day, dated, with the reason and the named
prerequisite. Worse, `docs/MASTER_COMPLETION_CONTRACT.md:185` still reads
*"Privacy horizon … open; do not infer from the old horizon park."*

> **The contract that owns the two-terminal-states vocabulary carries a row in a third
> state for a question its owner has already answered.** That is C-16 §3's own shape,
> inside the two documents a hostile reviewer would treat as authority.

Both are one-line repairs and are routed, not performed, below.

**And a corollary worth stating once**, because the entry list is careful about it and
readers will not be: an unwitnessed route is a statement about **coverage, not
correctness**, and C-16 closing is a statement about **readiness to be audited, not about
being finished**. The stopping condition in the contract adds an adversarial closure pass
*after* C-16. Conflating them would let a green C-16 read as completion.

---

## B1b. Materialize / Dematerialize → delete — **the conclusion survives; two of four grounds are FALSE and the decision deletes the wrong object**

### The label says the opposite of what it was read as — **FALSE ground**

`programs/dclutch-claims-sbf/src/lib.rs:597-602`, verbatim:

> `// ECONOMIC_SLICE_MIGRATION_ONLY: this generic ClaimsPlanV1 route remains`
> `// reachable solely for the current Trading General child-packet builder`
> `// (dclutch-general-adapter-contract/src/child_packets.rs) and Dealer`
> `// physical composers (dclutch-trading-sbf/src/dealer/physical.rs and`
> `// dclutch-dealer-sbf/src/lib.rs). New families use LBV2 affine/signed`
> `// plans; deleting those three consumers permits deleting this route.`

It does not mean "only usable during a migration." It marks the route legacy **for new
families** and names **three live consumers**, and it says deleting *them* is what permits
deleting the route. Read as a licence to delete, the label says the opposite of what it
says.

### "C-08's clause is already carried by Reconstitute/UnwrapStructured" — **FALSE ground**

They are two different accounting systems.

- **Materialize/Dematerialize** (`crates/dclutch-economic-slice-kernel/src/lib.rs:156-158`,
  `crates/dclutch-claims-svm/src/lib.rs:201-204`) move claim atoms for one outcome between
  the `native` and `materialized` `u64` pair on the Market record and the matching pair on
  two Position records, under the invariant `supply == native + materialized` (`:851-866`),
  with `supply` and Hoard unchanged (`:754`). **No token minted, no Custody effect, no
  Token-2022 CPI.**
- **Reconstitute/UnwrapStructured** run on the **LBV2 Position layout** via
  `AffineBatchPlanV2`/`SignedDeltaPlanV3`
  (`programs/dclutch-claims-sbf/src/rational_representation_v2.rs:9-12`).
  `crates/dclutch-liability-basis-v2-kernel/src/lib.rs` contains **zero** occurrences of
  `materializ`. `uses_claims()` is false for `IssueStructured`/`UnwrapStructured`.

C-08 (`docs/MASTER_COMPLETION_CONTRACT.md:93`) names *"unwrap **or dematerialize** …
through real Token-2022 and Custody effects"* as two alternatives, and only the unwrap
half is carried.

**The conclusion "nothing is lost from C-08" still holds — for the opposite reason.**
Because Materialize/Dematerialize produce no Token-2022 and no Custody effect, they could
never have satisfied that clause either. The codec was the missing half of that design —
its `AdapterStyle::{Mint, Burn}`
(`crates/dclutch-claims-representation-codec/src/lib.rs:80-89`) is exactly the token
effect that would have paired with the economic intent — and the two halves were never
wired to each other.

### "Zero dependents" is TRUE and misattributed

Confirmed across **all 169 manifests and all 74 `Cargo.lock` files**, nested workspaces
included: the only three hits are the workspace member line (`Cargo.toml:91`), the crate's
own name, and its own lock stanza. 1,444 LOC confirmed.

**But the codec does not implement the actions being deleted.** The on-chain
Materialize/Dematerialize/RedeemMaterializedTerminal live in `dclutch-economic-slice-kernel`
+ `dclutch-claims-svm` + `programs/dclutch-claims-sbf`, none of which depend on the codec.
The 1,444-line figure is being used to argue for a deletion it does not measure.

### "Constructed nowhere, driven by no test" — HALF FALSE

No construction of the `ClaimsAction` wire tags anywhere: all five `ClaimsPlanV1::new`
sites and the three live consumers build only `TransferNative`, `RedeemNativeTerminal`,
`MintCompleteSet`, `MergeCompleteSet`. **But the identical semantics is driven twice** —
`crates/dclutch-economic-kernel/src/lib.rs:719,734` constructed at `:1645-1646,1737` and
asserted byte-exact on pre-state, post-state, Claims plan **and** Custody plan against
Lean-emitted vectors (`formal/dclutch-semantics/vectors/economic-kernel-v1.txt:13-20`),
plus five slice-kernel frames (`crates/dclutch-economic-slice-kernel/src/lib.rs:1480-1560`)
including three hostile rollbacks. The true narrower statement: **no test drives the wire
tag or the SBF dispatcher arm.**

### The route is LIVE and admitted — this is what the decision misses

`programs/dclutch-claims-sbf/src/lib.rs:603-604` is an unconditional terminal fallthrough
into `process_generic_plan`, and `:625,626,628` map the three tags straight to
`BasketAction`. **No `#[cfg]`, no feature gate, no weld.** Every Claims ELF built from this
tree admits Materialize today for any registry-authenticated Claims-role caller.

> **Deleting the codec alone leaves an admitted route with its Rust owner half-removed —
> strictly worse than either endpoint.** The "live, unexercised supply-moving code" is at
> `claims-sbf:625-628` + `slice-kernel:808-830`, not in the codec.

### Nobody is stranded, and the cut must be one act

`materialized` is credited in exactly two places tree-wide, both gated on Materialize
(`slice-kernel:811-813` basket path, `:336-337` command path). Nothing else can put a
holder into the materialized representation, so **no reachable state requires Dematerialize
to exit.**

The coordinated cut: the dispatcher arm (`claims-sbf:625-628`); `ClaimsAction` tags
**1, 2, 4 retired as holes and never backfilled** — the enum is `#[repr(u8)]` with explicit
discriminants 0..7 (`claims-svm/src/lib.rs:198-216`), so renumbering would silently change
the wire for the four live actions; the `BasketAction`/`Command` variants; the
`Plan::materialize_claim` constructors and the `State` fields
`materialized_supply`/`source_materialized`/`destination_materialized`; and the Lean vector
rows regenerated from `EmitEconomicVectors.lean`.

**Nothing is withdrawn and nothing moves in the censuses.** No `ClaimsSbfError`
discriminant is owned by these actions (they route through generic
`Instruction`/`Accounts`/`Economic`), `crates/dclutch-refusal-registry` has zero
`materializ` hits, and `docs/reference/` and `tools/gauntlet/census/` have none either —
**no NEVER-EXECUTED row disappears, because none was ever written.** The codec's own
deletion is a disjoint set: 1,444 Rust LOC plus **1,204 Lean LOC** across four
`formal/dclutch-semantics/DClutchSemantics/ClaimsRepresentation*.lean` files, plus nine
mirror/manifest rows of which `packages/dclutch-sdk/lib/capabilityModel.ts:226-228` is
hand-maintained and would cite a nonexistent path.

**Browser check: the accused shape is NOT present.** `apps/dclutch-web` does render
`'Materialize'` (`lib/explorer/accountRecords.ts:790,810`) but from
`ACTION_MATERIALIZE_V2/V3` in `lib/generated/generalSuccessorV5.ts:108,115` — **General
successor V5 vocabulary, a different family**, alongside `GENERAL_PHASE_MATERIALIZING_V2`.
The Claims representation wire has no browser mirror. `AGENTS.md`'s last-authority hazard
does not fire here.

### What is genuinely lost, and it is a decision rather than a deletion

`docs/evidence/ASPIRATION_LEDGER_2026_08_27.md:2481-2487` (**N-11**) records that the hybrid representation —
internal Position plus optional Materialize/Dematerialize — *"is built and shipped three
generations deep… and the reject-criterion was never evaluated."* `:1168` (**M-52**) is
external venue routing for materialized claims, whose stated rationale for the hybrid was
exactly this. **Deleting is the reject decision for N-11 and should be recorded as one**,
not as tidying.

### C3 addendum — the published register and the measured figure differ, as documented

Measured at `b6b14ab2`: `docs/reference/routes.md` has **161 route rows** and **57**
`NEVER-EXECUTED, no stated reason` rows (58 grep hits minus the legend at `:27`). The
measured figure is **55**, after the two folds at `939d0806`. That is exactly the
staleness `C16_ENTRY_LIST_2026_09_01.md:41-45` declares deliberately — the convergence
owner is holding regeneration until the lanes quiesce. Recorded so a reader who greps the
published register and gets 57 knows why, and does not "correct" the 55.

## A4b. Claims split/merge as user acts — **the decision SURVIVES: build it. Two premises in the brief are already stale.**

### The orphan is no longer an orphan

`crates/dclutch-claims-conservation-contract` is **1,647 LOC** (`src/lib.rs` 896 +
`src/tests.rs` 751) and has **a real Cargo dependency**:
`crates/dclutch-operator/Cargo.toml:31`. `cargo metadata --no-deps` shows 106 workspace
members with the crate among them and `dclutch-operator` depending on it. It landed at
`1d1c2453` (2026-09-01 08:53), 4 files, **+384/−0**, adding
`crates/dclutch-operator/src/claims_conservation_v1.rs` (379 LOC,
`plan_claims_conservation_v1` at `:125`).

**Two documents say otherwise and are now false:**
`docs/evidence/DEBT_OWNERSHIP_LEDGER_2026_09_01.md:201-204` (*"it appears in exactly one
place in the tree — its own declaration… no operator builds it"*, both clauses) and the
crate's own header at `crates/dclutch-claims-conservation-contract/src/lib.rs:49`
(*"no operator builds it"*).

The orphanhood that survives is at the **call site**: `plan_claims_conservation_v1` has
zero callers outside its own tests. But `dclutch-operator` has 12 consumers including
`crates/dclutch-user-position-admission-wasm/Cargo.toml:19`, the browser path — so the
planner is **one hop from a client**.

### The contract is correct, with one residual it names itself

`quantity * basis_scale` exact-or-refuse (`lib.rs:324-332`, no rounding, no saturation,
byte-identical to `crates/dclutch-claims-svm/src/founding_v5.rs:278-281`). Compartments
pinned (`:299-313`: Split `External → HoardPrincipal`, Merge the reverse; consumed at
`:698-699`; `hoard_vault_seeds:641-648`). "Neither half constructible without the other"
means: `custody_request` (`:669`) and `write_uniform_quantities` (`:790`) are methods on
one struct and each calls `self.validate()?` first (`:670`, `:791`), and `validate`
refuses unless `collateral_atoms == quantity * basis_scale` (`:551-555`) **and** both
stated balance deltas equal exactly that (`:558-580`).

**32 tests, 32 passing** (7.2 s, filtered). 19 assert a refusal, every one naming the
**exact** `Error` variant — no bare `is_err()`. Fixtures deliberately run
`BASIS_SCALE=11, QUANTITY=7, COLLATERAL=77` (`tests.rs:17-20`) so the unit bug is visible.

**The residual the crate states about itself** (`lib.rs:831-834`): *"An adapter that
commits one without the other has produced the hole this crate exists to close."*
Constructibility is coupled; **emission is not**. That gap is exactly the outer route's
job — an argument *for* building it.

### The `MintCompleteSet` accusation is CORRECT, and worse than stated

`GENERIC_ACCOUNT_COUNT = 13` (`programs/dclutch-claims-sbf/src/lib.rs:92`), indices 0–12
at `:66-90`: authority, market, two positions, activation cache, caller program +
programdata, claims program + programdata, registry, core market + program + programdata.
**No token account, no mint, no token program, no vault, no custody program.** The data
layout (`crates/dclutch-claims-svm/src/lib.rs:240-252`) carries no amount-in-atoms, no
mint, no `basis_scale`. `execute_plan_economics` (`claims-sbf/src/lib.rs:1049`) contains
zero `invoke`/token/custody references, and the whole Claims program constructs exactly
**two** `CustodyRequestV1` — `custody_replay_v1.rs:181` and `rational_terminal_v3.rs:351` —
neither on the complete-set path.

**What binds it is one thing, and it is not collateral.** `authenticate_complete_set_growth`
(`lib.rs:1382-1399`) calls `MarketPrincipalCapSetsV1::admit_growth`, which is
`outstanding + added ≤ cap`
(`crates/dclutch-source-contract/src/principal_capacity_v1.rs:489-495`). **The cap is a
static policy `u128` projected at founding, not a reading of the Hoard vault's balance.**
A ceiling on issuance, not a collateral binding.

**Mitigation, not refutation:** the generic route is CPI-only — see the composition check
below — so this is not today a user-reachable mint-from-nothing. **And a second fact both
audits converged on independently: `MintCompleteSet`/`MergeCompleteSet` have no production
constructor at all.** The sole builder in the tree is `build_materialize_packets_v2`
(`crates/dclutch-general-adapter-contract/src/child_packets.rs:489-535`) whose only caller
is its own unit test at `:926`; `programs/dclutch-dealer-sbf/src/lib.rs:1915-1946` builds
only `TransferNative` and `RedeemNativeTerminal`.

### The magic and the dead code — confirmed

`DCLCNS01` (`lib.rs:166`) has **zero occurrences anywhere under `programs/`**. The
dispatcher that does not list it is `process_remaining_instruction`
(`programs/dclutch-claims-sbf/src/lib.rs:447-608`), which matches 13 magics by prefix and
then falls through to `ClaimsPlanV1::decode` at `:605`, which refuses on magic.
`CustodyRequired = 0x5006` (`:182`) occurs only at its declaration, its `ALL` entry
(`:272`) and its exhaustive `ordinal` arm (`:296`) — **no raise site** — while being
published to readers at `docs/reference/refusals.md:66`,
`docs/reference/abi/refusalRegistryV1.md:167`, `routeCensus.md:215`, and both generated
`refusalRegistryV1.ts:141`. The reader-facing census lists a code no route can emit.

### The wall is honest but scoped narrower than the truth

`packages/dclutch-sdk/lib/capabilityModel.ts:219-222` declares `claims.conserve` with
*"No signature is requested, because nothing here can build this transaction"* and a wall
citing the contract crate — enforced by `apps/dclutch-web/lib/capabilityEvidence.test.ts:152-171`
(every no-venue act needs a wall whose citation resolves) and `:174-183` (a `FORBIDDEN`
list banning "coming soon", "roadmap", "temporarily"). Both clauses are true.

**But the wall never says the on-chain dispatcher is missing**, so a reader concludes the
wire is live and only the UI is absent. The sibling `claims.represent` (`:223-227`) carries
a *second* wall naming the real campaign wall; `claims.conserve` carries only the
client-side one. Two soft spots publish merge arithmetic hardcoding `basis_scale == 1` with
no wall beside them: `apps/dclutch-web/lib/portfolio.ts:161` and
`components/charts/PositionBars.tsx:143-145,74`.

### The composition check — no path admits split/merge today

I looked for a case where the accused shape is admitted, four ways, before concluding.

1. **Direct user call — impossible.** `authenticate_authority`
   (`programs/dclutch-claims-sbf/src/lib.rs:1215-1243`) requires
   `accounts.authority == find_program_address(CallerAuthoritySeedsV1, caller_program)` — a
   PDA *of the caller program* — and `authenticate_generic_privileges` (`:1134`) requires
   it to sign. A wallet can never be that key. The generic route is CPI-only.
2. **Core effect `SplitClaims` — arm exists, producer does not.** The dispatch arm is live
   at `lib.rs:695`, but nothing constructs a `CoreEffectEnvelopeV1` with
   `CoreEffectActionV1::SplitClaims`; the only constructions tree-wide are `AdmitTerminal`,
   `CloseFund`, `CreateFund`, `VerifyFundReady`, `ActivateCapability`, `CloseCapability`.
3. **General/Dealer settlement — the one constructor is test-only** (above).
4. **Repeated founding — gated shut.** `founding_v5` requires the aggregate, position and
   admission accounts to be **vacant** (`founding_v5.rs:1100-1105`) under
   `CorePhaseGateV3::Exactly(CorePhase::Founding)` (`:1058`). Once per market.

> **Split/merge is unreachable by every path. The decision is "build", not "document."**

### What the outer route costs

Already paid: the contract (1,647 LOC, 32 green) and the planner (379 LOC, `1d1c2453`).
Remaining: SBF handler + dispatcher arm + client mirrors + on-chain tests.

- **Instruction:** new `programs/dclutch-claims-sbf/src/claims_conservation_v1.rs`;
  selector is the existing, unique `DCLCNS01` (no `magic-collisions.json` entry needed);
  ~6 dispatcher lines beside the custody-replay match at `lib.rs:481`. One line into
  `programs/dclutch-claims-sbf/Cargo.toml`.
- **Accounts:** the 13 generic, **plus the collateral leg today's route lacks** — Hoard
  vault (`hoard_vault_seeds:641`), the actor's external token account, collateral mint,
  token program, Custody program, Claims-role Custody replay
  (`custody_replay_seeds:630`), Custody transfer-authority PDA. Authority becomes the
  Position **owner signing directly**, which is the material change from today's CPI-only
  authentication.
- **Token-2022:** **no claim-token mint or burn** — native claims are Position scalars, not
  SPL mints. Exactly one collateral `transfer_checked` via Custody, mint and token program
  realm-validated (`programs/dclutch-custody-sbf/src/lib.rs:1396`). **No `closeable_mint`**
  — those are Fractional/Rational constructs. Split must use the delegated V2 wire (Custody
  V1 `Transfer` refuses an `External` source, `custody-sbf/src/lib.rs:1389-1391`), and that
  wire is **already live and dispatched** at `custody-sbf/src/lib.rs:275-278`.
- **Refusal codes: none new.** `CustodyRequired = 0x5006` is already reserved and finally
  raisable; `PrincipalCapacity 0x500D`, `Economic 0x5005`, `Authority 0x5004`,
  `Identity 0x5002`, `Accounts 0x5001` all exist.
- **Mirrors:** an `abi:claims-conserve` **+ `:verify`** pair, generator and
  `lib/generated/*.ts`, **doubled** across `apps/dclutch-web/package.json:14-58` and
  `packages/dclutch-sdk/package.json:25-69` — `apps/dclutch-web/lib/abiVerification.test.ts:57-79`
  reds on a generator with no verify sibling. The route census **auto-discovers** a new
  dispatch arm (`tools/gauntlet/census/src/enumerate.rs:3-5`); `bindings.json` and
  `CU_BUDGETS.json` are hand-kept. And `capabilityModel.ts:219-222` flips from wall to
  anchored action.
- **Tests:** an on-chain `programs/dclutch-claims-sbf/program-test/conservation/`.
  **The honest control must run at `basis_scale ≠ 1`** — every in-tree fixture uses 1,
  which is exactly what hides the merge unit bug.

**Cost basis, measured with `git show --numstat`:**

| precedent | commits | files | +ins |
| --- | ---: | ---: | ---: |
| `founding_v5` (SBF+codec only) | 1 | 4 | 1,647 |
| `terminal_settlement_v3` | 2 | 8 | 1,838 |
| **`custody_replay_v1`** — closest comparable (Claims top-level building a `CustodyRequestV1`, with web mirror) | 4 | 20 | **1,857** |
| `retirement_replay_handoff_v1` | 5 | 18 | 3,122 |
| `dealer_reservation_v1` (most recent Custody instruction) | 6 | 31 | 4,947 |
| the operator half, already landed (`1d1c2453`) | 1 | 4 | 384 |

**Estimate: 4–6 commits, 20–30 file touches, +2,500 to +3,500 insertions.** Floor is
`custody_replay_v1` (no delegated-allowance leg, no cap check); ceiling is
`dealer_reservation_v1`, but that introduced new contract *types*, which this does not.

**Two process facts that will bite:** mirrors are never in the program commit (always a
separate commit, and `docs/reference/` is a separate sweep days later), and
`tools/genref/generate.sh --check` runs on **no CI tier** (`grep -c genref tools/ci/run.sh`
= 0), so reference drift is caught only by whoever remembers.

### Three qualifications to carry, none of them blockers

1. **Throughput, named by the crate itself** (`lib.rs:625-636`): reusing the Claims-role
   Custody replay coordinate makes a per-Market serial `u64` the throughput bound once
   split/merge is public. Rare for terminal payout; not rare for split/merge. Deliberately
   accepted over forking the cursor.
2. **An inherited unproven premise** (`lib.rs:77-146`): `Σ_k claims_payout[k] == basis_scale`
   is not enforced on the generic settlement route, and founding pins no exposure identity.
   The crate is right that a split is exactly as sound as the founding before it — but the
   route inherits the gap. Sequence it; do not let it gate. (Same defect as §C1's tail.)
3. **The legacy handler should die with this campaign.** `MintCompleteSet` is a live
   handler with a principal cap and no collateral coordinate, and it has no production
   constructor. The honest end state deletes the uncollateralized route in the same
   campaign that lands the collateralized one — which is also `AGENTS.md:284-285`'s rule
   about parallel authority paths.

## B4. "Is a K = 2 structured product useful?" — **DISSOLVED as a product ruling. C-03 already answers it, and the answer is no.**

The coordinator is right that this is a measurement rather than a ruling, but the decisive
measurement is not the packet arithmetic. It is a width constraint the curve family
already enforces.

**An admitted degree-2 curve cannot exist at K = 2.** The chain, each link checked:

1. `BasisKindV3::SplineDegree2To3` makes a price-gate certificate **mandatory**:
   `crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs:467-475` refuses
   `PriceGateCertificateRequired` when the digest is all-zero — *"the whole of this
   family's degree interval is above the gate's exempt degree, so a certificate is
   mandatory and its absence is refused here rather than at settlement."*
2. The certificate refuses unless its width is **strictly above its degree**:
   `crates/dclutch-product-payoff-v2-codec/src/price_gate_v1.rs:164` —
   `if width <= usize::from(degree) || width > PRICE_GATE_MAX_WIDTH_V1 { PriceGateWidthOutOfRange }`.
3. That width is joined to the basis's own: `PriceGateBasisMismatch`
   (`runtime_v3.rs:179-181`) is *"issued against different founding-fixed quantities than
   the basis carries — a different scale, degree or width."*
4. Founding pins the basis width to the outcome count:
   `programs/dclutch-core-sbf/src/generic_founding_v1.rs:1090` refuses
   `CoreSbfError::Reference` unless `basis_width == runtime.outcome_count`, and `:1096-1100`
   sets `claim_count` from that same width.

**There is a harder refusal than the price gate, and it is worth naming because it is
earlier.** `crates/dclutch-product-payoff-v2-codec/src/spline_eval_v3.rs:226,265-267`
iterates `for span in degree..width`, which is an **empty range** when `width <= degree`,
so `selectable == 0` and it returns `SplineDegenerateSpan`. That is reached
unconditionally from `ProductBasisV3::validate` (`runtime_v3.rs:564`), so **a K = 2 spline
record cannot decode on chain at all** — it does not reach the certificate. Two further
independent sites carry the same floor: `spline_eval_v3.rs:453` (`offset + len > width`,
`len == degree + 1`) and `crates/dclutch-liability-basis-v2-kernel/src/spline.rs:246-250`
(`knot_count >= 2*degree + 2`). And `BASIS_SPLINE_MINIMUM_DEGREE_V3 = 2`
(`generated_runtime_v3.rs:13`), so **every** admitted curve is degree ≥ 2.

### The C-03/C-08 tension is real but narrower than "the rows conflict"

C-08's **five unwitnessed routes all close fine at K = 2** —
`UNWITNESSED_ROUTES_BY_ROW_2026_09_01.md:88-96` lists them and every one is an
`asset_count == 1` retirement or close route (`fractional_retirement_v3` ×4,
`rational_representation_v2::process_replay_close`). None is width-sensitive.

What cannot close at K = 2 is **campaign 5**
(`docs/MASTER_COMPLETION_CONTRACT.md:121-123`): *"compile an admitted **nontrivial
curve** → found → **issue** exact fractional representation → …"*. And the constraint is
**specific to the Structured child wire**: `crates/dclutch-fractional-claim-operator/tests/topology_v3.rs:72-91`
(run filtered, green) shows every Fractional route at **maximum width** fitting with room
— WrapOrWholeUnwrap 682, TerminalRedeemOrZeroBurn 708, Terminalize 656, retirement
508/536/512 — because none carries a per-coordinate wire tail. **So C-03's curve work can
close on the Fractional wire; it is `IssueStructured`/`UnwrapStructured` alone that the
packet caps.** I had written this more broadly on my first pass and it was too strong.

**This still stacks with B6.** The partition gate refuses any single-ordinary-cell
partition (one cell always scores 10,000 bps against a ceiling of at most 10,000), so a
width-2 market — the protocol's own documented floor — is refused at founding through
`compile_market_bodies` anyway. Width 2 is squeezed from below by the partition gate and
from above by the curve family's degree floor, and neither squeeze is a preference.

### The packet arithmetic, measured — and K = 2 is not the harbour it reads as

Reproduced at HEAD, filtered:

```
Rational V2 K=3 packet wall: full-width-v0-live-ALT=1397, selected-v0-live-ALT=1061,
limit=1232, over-by=165, per-coordinate=168, executable-full-width-K=2
```

**1,397 is the honest figure; 1,357 is stale.** `7b80869d` (today) made every wire
measurement carry `set_compute_unit_limit`, and its own message tabulates
`1357 → 1397 (+40)` — the ComputeBudget program id entering static keys (32) plus its
compiled instruction (8). *A packet figure that omits what a real transaction carries is a
packet figure for a transaction nobody sends.* **The overrun to close is 165 bytes, not
125.**

A byte model reconstructed from `live_lookup_v0_wire_bytes` reproduces **both** the 1,397
and the 1,061 exactly, which pins every count: 129 signatures, 1 version, 3 header, 129
static keys (4 × 32), 32 blockhash, 1 ix count, 8 `SetComputeUnitLimit`, **1,018 for the
Issue instruction** (45 account indexes + 969 data), 75 ALT (9 writable + 32 readonly).
**768 of 1,397 bytes — 55% of the packet — are 32-byte pubkeys carried inline inside the
request.**

| lever | bytes | verdict |
| --- | ---: | --- |
| more accounts into the ALT | **0** | exhausted — 41 of 45 already looked up; the other 4 are structurally ineligible (`solana-message-4.5.0/src/compiled_keys.rs:145-150`: payer and actor are signers, the program and ComputeBudget are invoked) |
| a second ALT | **−34** | strictly negative (32 key + 2 length, relocating zero indexes) |
| merge actor into payer | **+96** | one signature and one static key; `1397 − 96 = 1301`, still 69 over |
| split Issue and Unwrap | **n/a** | already separate — the measurement is one `IssueStructured` alone; `request.rs:477-481` forces `asset_count == outcome_count` for both, which is also what blocks splitting one issue across K transactions |
| **drop the 3 re-derived per-coordinate PDAs** | **−288** | → **1,109, fits with 123 to spare**. `programs/dclutch-claims-sbf/src/rational_representation_v2.rs:1157-1203` already derives all four coordinate accounts from `(program_id, descriptor, outcome)` and then requires the inlined copies to equal them |
| … plus the two mandatory-zero terminal fields | **−64** | → **1,045**, per-coordinate cost 168 → 72, executable full width rises to **K = 5** |

> **K = 3 cannot fit under any lever that leaves the request ABI alone: the three ABI-free
> levers sum to at most 96 bytes against a 165-byte overrun. K = 3 fits if and only if the
> 32-byte pubkeys stop riding inside the request.**

That is an ABI revision, not a code tweak — `generated.rs:1` is *"@generated by
`formal/dclutch-semantics/EmitRationalRepresentationV2PhysicalAbiRust.lean`; do not
edit"*, and the request is also the digest preimage for the caller-authority PDA. It is
also the tree's own named lift: `open_structured_v3.rs:116` — *"The lift this cliff wants
is commit-don't-inline or staged issuance, not a wider record."*

**And the harbour is three bytes wide.** `full(K) = 1061 + (K−1)·168`, so **K = 2 is 1,229
against 1,232**. Nothing in the tree records that margin. Meanwhile the house packet
builder — `crates/dclutch-representation-composition-v3-operator/src/lib.rs:861-893`,
whose own doc says *"Both compute-budget instructions are included in packet
accounting"* — pushes `set_compute_unit_limit` **and** `set_compute_unit_price`
unconditionally, and the second compiles to 12 bytes. **1,229 + 12 = 1,241 — nine bytes
over.** So "K = 2 fits" holds only for a transaction that pays no priority fee. *(Derived
from the serialization, not measured on a built packet; the same arithmetic yields the
8-byte limit instruction that makes the model hit 1,397 exactly.)*

**K = 2 was executable once, and the fit has never crossed a cluster.** At
`2f02316435a6bad6e3b188705604801898e732d9` the campaign ran `OUTCOME_COUNT = 2`,
`COEFFICIENTS = [3, 7]`, `DENOMINATOR = 10`, eleven tests on real ELFs
(`docs/decisions/0011-structured-v2-physical-route.md:604-609`;
`tools/gauntlet/claims-rational-representation-v2/witnesses.json:51` — *"It was eleven at
K = 2"*). `429b6747` moved it to K = 3 and overwrote it, so "no K = 2 route exists" is
true of HEAD but not of the history. **Two caveats that matter more than the archaeology:**
ProgramTest has no MTU — `429b6747` says so — so that execution proved semantics and never
that a 1,229-byte packet crossed a real cluster; and the current tree has no K = 2
structured fixture at all.

**The bound itself has no owner.** `1232` / `1_232` occurs **115 times** across `.rs`,
`.ts` and `.tsx` at HEAD, including at least five independent private constants in the
browser alone (`apps/dclutch-web/lib/{claimsCustodyReplay.ts:105, rpc.ts:21,
solanaLimits.ts:2, walletTerminalPayoutV3.ts:200, directTradeSpine.ts:133}`).
`AGENTS.md` requires every fixed bound to be labeled mathematical, chain-derived,
measured-profile or provisional; the sibling PDA bounds *are*
(`crates/dclutch-account-profile-contract/src/lifecycle_v3.rs:75,77`) and the packet bound
is not, anywhere. `apps/dclutch-web/lib/solanaLimits.ts:2` is the obvious owner for the
browser half and four of its five siblings do not use it.


---

## Repairs routed, not performed

House law forbids this lane from editing production code. Each of these is a `file:line`
for its owner.

**Corrections to authority documents** — these are C-16 §3's own category, sitting inside
the documents a hostile reviewer would treat as authoritative:

1. `docs/MASTER_COMPLETION_CONTRACT.md:185` — the Privacy-horizon row reads *"open; do not
   infer from the old horizon park."* Ember ruled C-15 **out of the accepted project** at
   `5a371810`, 2026-09-01, dated and with a named prerequisite. The contract's own
   vocabulary clause forbids a third state; this row is in one.
2. `docs/MASTER_COMPLETION_CONTRACT.md:187` — the recovery row asserts *"the per-leg
   `FailNext` walk sits inside `#[cfg(any())]`, so the ELF builds because the block never
   compiles."* **No live `#[cfg(any())]` attribute exists in the tree.** The row's
   conclusion survives on a live premise (`exhaust_after_primary_deadline`'s refusal at
   `crates/dclutch-source-contract/src/source_resolution_v2.rs:466-467`); its stated
   mechanism does not.
3. `docs/evidence/C16_ENTRY_LIST_2026_09_01.md:417` — lists R-8 as open, 45 minutes before
   ember ruled it.
4. `docs/evidence/DEBT_OWNERSHIP_LEDGER_2026_09_01.md:201-204` and
   `crates/dclutch-claims-conservation-contract/src/lib.rs:49` — both say *"no operator
   builds it"* about `DCLCNS01`. False since `1d1c2453`, this morning.

**Comments that describe deleted code** — the same class C-16 §4b instruments, in three
new places:

5. `programs/dclutch-trading-sbf/src/entrypoint_adapter.rs:1298-1308` — justifies
   `DCLTHOT3`'s extended-heap declaration by *"two Registry reauthentication CPIs"* that
   decision 0017 option B removed, and which `hot_v3.rs:4880-4882` says outright *"It
   stopped."* **A ruling was taken on this comment.**
6. `packages/dclutch-cli/src/main.ts:180` and `tools/dclutch-cli/src/main.rs:140-141` —
   both assert `@dclutch/cli` is on npm. It is `private: true` and 404. **These are almost
   certainly the origin of the backwards binary decision**, because they are the two files
   anyone would open to check.
7. `programs/dclutch-core-sbf/src/resolution.rs:878-882` — a lifting plan whose first step
   is *"resurrect"* a symbol with no definition. Already in
   `tools/doc-citations/baseline.json:11-16`; recorded here because it is load-bearing for
   the recovery ruling rather than cosmetic.

**Live defects found while reading:**

8. `tools/local-validator/bootstrap/successor/src/relayed.rs:532` sets
   `founding_band: None` and `tools/local-validator/bootstrap/successor/src/market.rs:3172`
   requires one unconditionally, so the relayed graduation market **cannot be founded**
   through the path its own release story names
   (`tools/devnet-scenarios/fixtures/graduation.json:675`). Not ungated — bricked.
9. `tools/local-validator/bootstrap/successor/src/market.rs:12283-12290` documents a
   zero-cut `LocalMarketShapeV1` as *"legal and the narrowest market this compiler can
   emit."* That capability is dead at HEAD: one ordinary cell always scores 10,000 bps.
10. `programs/dclutch-general-accelerator-sbf/src/lib.rs:440-459` names a variable
    `heap_granted` and sets it from the ComputeBudget **request** — a fourth instance of
    the class, harmless in that program but the same defect.
11. `programs/dclutch-trading-sbf/src/entrypoint_adapter.rs:287` (`HEAP_HEADER_BYTES = 24`)
    disagrees by 752 with `docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:3632-3634`'s *"the bump heap reserves 776 bytes at its
    floor."* Since the scratch bumps **down from the ceiling**, a fault at `ceiling − 776`
    is more simply the first scratch block being 776 bytes wide. The observation stands;
    the attribution looks wrong.
12. `apps/dclutch-web/lib/explorer/transaction.ts:189-190` — an access violation reaches
    the user as `InstructionError #3: ProgramFailedToComplete` while
    `0x4008 TradingSbfError::HeapFrame` has a full meaning string in both generated
    registries. This is the concrete cost of choosing "abort" over "refuse" in the heap
    ruling, and it is not in the ruling's statement.
13. The 1,232-byte packet bound is restated **115 times** with no owner and no
    chain-derived label (see §B4).
14. **Two gauntlet witnesses are red at HEAD and nobody has noticed.**
    `tools/gauntlet/structured/witnesses.json:22` and
    `tools/gauntlet/claims-rational-representation-v2/witnesses.json:19,22` both assert
    `"expect": "1357"` as a **deliberate equality** — the structured one says so in its own
    provenance: *"This witness is deliberately an EQUALITY, not an inequality: an island
    whose only packet claim is 'still broken' would not notice the frame getting worse."*
    `7b80869d` moved the figure to 1,397 and updated neither. **The witness did exactly
    what it was designed to do and its author was not there to read it.** Three more prose
    sites carry 1,357: `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs:5215,5217`,
    `crates/dclutch-structured-v2-operator/src/child_request.rs:80` and
    `crates/dclutch-bearer-v2-operator/src/open_structured_v3.rs:109`, plus
    `docs/decisions/0011-structured-v2-physical-route.md:630-633`.
15. `docs/ledger/LETTER_TO_CLAUDE_2026_09_01.md:672` conditions a standing instruction on the
    stale figure — *"If K=3 `IssueStructured` remains at its measured 1,357 bytes, keep
    that browser act blocked; use a proven K=2 route…"*. Both halves are now wrong: the
    figure is 1,397, and **no K = 2 route exists at HEAD** (it existed at `2f02316`
    and `429b6747` overwrote it). The instruction's *conclusion* — keep the browser act
    blocked — is more true than before, not less.

**Repairs with a route already named:**

16. §C1 — carry `ProductBasisV3::payout_scale` into `PoolEquityInputV3` by authenticating
    the Product record against `core.identity.product_record`, which
    `programs/dclutch-trading-sbf/src/dealer/v3_accelerator_accounts.rs:441-467` already
    loads and pins. Also touches `v3_composer.rs:323-335`.
17. §A1 — the invariant *"a position's address commits the terms it was admitted under"*
    is enforced today and written down nowhere; it belongs beside
    `CapabilityRootSeedsV1::new`
    (`crates/dclutch-capability-program-contract/src/lib.rs:759`).

---

## What I verified, and what I did not

**Verified by reading source and running filtered tests at
`61817d7a`…`b6b14ab2`:** every `file:line` above; the config-digest-in-PDA-seeds chain
(§A1); `generation`'s immutability and the absence of any setter; the red state of
`the_solvency_gate_must_not_change_its_verdict_with_the_claim_unit` with its exact message;
`basis_scale`'s founding pin and `CoreState`'s lack of a scale field; the absence of any
live `#[cfg(any())]` attribute and of any `process_funded_transition` definition; the
`ECONOMIC_SLICE_MIGRATION_ONLY` label's actual text; the `1..=10_000` ceiling bound at all
three sites; `FoundingBandV1`'s field list; `relayed.rs`'s `founding_band: None` against
`market.rs:3172`; both `dclutch` binaries' publish posture and their two false npm
comments; the spline degree-vs-width chain in §B4; the 161/57 route counts at HEAD.

**Taken from delegated fact-finding and not re-derived by me:** the GitHub release
download counts for the Rust binary; the Switchboard and Pyth web research in §A3 (cited
with URLs there, and its central negative — that Switchboard publishes no per-update
price — is a claim about an absence I did not independently sweep); the `git --numstat`
cost tables in §A2, §A3 and §A4b; the 169-manifest / 74-lockfile dependent sweep in §B1b;
the 32-test pass in §A4b.

**Not measured, and named as owed:** whether `DCLTHOT3`'s heap peak still exceeds 32,768
at HEAD (§B3 — one profiled run); whether any pre-weld recovery-bearing market exists on a
live cluster (§A2 — I verified only that none can be created now); and a built-packet
confirmation of §B4's 1,241-byte K = 2 figure under the house builder, which is derived
from the serialization rather than measured — though the same arithmetic reproduces the
1,397 and the 1,061 exactly, which is the positive control for the model.

## Addendum, 2026-09-04 — two verdicts, re-read against what was built

- **§C2's premise was inverted by the lane sent to build the repair** (the
  orchestrator's correction of 2026-09-01,
  `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L2446`): the Structured
  operator's route geometry was right and `claims_composition_v3.rs` was
  wrong — it refused any representation route that is not `Once`, and
  `AffineOnce` would bind K == N, which the family exists to deny. The four
  facts §C2 verified held; the inference drawn from them did not. The repair
  order §C2 ruled (Content/Route first, the coefficient guard last, with a
  control) was followed.
- **§B4 stands and was ruled**: decision 0029 item 4 refuses the bare width-2
  spot band and admits a stated proposition with a prior in its place;
  `docs/design/PACKET_LIMIT_2026_09_01.md` (head) records K=2 dissolved twice.
