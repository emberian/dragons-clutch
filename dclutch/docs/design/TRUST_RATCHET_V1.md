# Trust Ratchet V1: extending verify-once over the hot route's re-verification mass

Design, 2026-08-30. Owner of the argument, not of any code. Every CU figure is
labelled **measured** or **arithmetic**, and every refusal below is a refusal
rather than a deferral.

The charter is ember's ruling of 2026-08-30 (`WAVE.md`, *Rulings — afternoon*):

> **Trust should ratchet forward as state mutates.** Per-transaction
> re-verification of write-once, program-owned records is the part of the
> "trust nothing" posture that overreaches; caller-supplied data stays
> untrusted (O-016 stands). Extend the seal/verify-once pattern.

The target list is `docs/evidence/DIRECT_HOT_CU_VARIANCE_CENSUS_2026-08-30.md`
(commit `b61ffdad`), whose mass decomposition sizes a **seal / cache
amortisation** lever at 159,900 CU, 11.6% of the profiled transaction.

## 0. The answer, first

**The lever as the census scoped it is not there. About 60,000 CU is claimable
today with no new account, and it is 4.4% of the transaction, not 11.6%.**

| | CU | share of 1,375,003 | label |
|---|---:|---:|---|
| census's `seal / cache amortisation` lever | 159,900 | 11.6% | measured buckets |
| of that, already banked before the census was written | ~89% of the 50,761 bucket | — | measured, decision 0005 |
| honestly claimable now, **no new account** | **~60,000** | **4.4%** | 10,500 measured + ~49,500 arithmetic |
| further, needs one checkpoint to size and a P-006 close route to be responsible | strictly < 82,000–87,000 | < 6.3% | bracket, unmeasured |

Two corrections drive that:

1. **The census's second lever bucket is a post-seal residue, not an
   opportunity.** *"sealed artifacts + execution-strategy record + Effect
   decode" — 50,761 CU* is what remains of a bucket decision 0005 measured at
   **645,836 CU before** and **56,693 CU after** (`0005:394`). Counting it as
   headroom for "seal / cache amortisation" counts the same saving twice.

2. **The single largest verify-once opportunity on the route is in a bucket the
   census assigned to no lever at all** — *"Registry reauthentication CPIs ×2 —
   52,592 CU"*. Its ratchet carrier already exists, already ships, is already
   the *only* path on the continuation route, and needs no new account, no rent
   and no packet byte.

And one finding that is not about compute at all: **the shipped capability seal
already carries a staleness proposition whose only guard is a single condition in
a function that does not know it** — `require_prefunded_vacant(frame.raw)` at
`programs/dclutch-registry-sbf/src/record_v1.rs:342`. §7 states the reachable
failure it forecloses. Nothing is wrong today; the guard should be named before
anything else is built on top of it, and before P-006's close-route work goes
anywhere near record accounts.

**STATUS AT 2026-08-31 (LEDGER-TRUE) — this section's "claimable" is now
"claimed", and the sequencing it asked for was honoured.** R-1 shipped as
decision 0017 option B in `1da601e7` and measured **−66,921 CU**, not the
~49,500 predicted (`0017:214`; §3's amendment and §8.2 carry the reason the
estimate was low). So the table's *"honestly claimable now, ~60,000, 4.4%"* row
is banked and then some: the realised figure alone exceeds the whole
two-candidate estimate, and **R-2's 10,500 is what remains outstanding of the
60,000**. The fourth row's two preconditions have separated: the checkpoint is
still unbuilt, but *"a P-006 close route to be responsible"* is **satisfied** —
P-006 is `CLOSED 2026-08-31` with the beneficiary ruled (closer, capped), so R-3
is no longer gated on it. And the guard did get named before the close-route
work: §7.3's tripwire landed the same night, from the same lane.

## 1. What a ratchet is here, and the rule that sorts the candidates

### 1.1 One correction to how the pattern is usually described

`borrow_sealed_record` (`programs/dclutch-trading-sbf/src/hot_v3/seal.rs:462`)
**derives nothing and hashes everything.** It skips both
`find_program_address` calls, because the seal persisted the canonical
raw-record and staging addresses; it still recomputes
`solana_program::hash::hash(&data).to_bytes() != digest`
(`seal.rs:494`) on every use, along with owner, privileges, rent exemption and
exact width. What it *does* consume from the seal is two addresses and a verdict
— and one further proposition, about an account it does not carry, which §7 is
about.

That is not an implementation detail, it is the whole soundness argument
(decision 0005, *"Why that is not a weakening"*, step 1): the sealed verdict is
about **bytes**, so the bytes must be re-pinned to their own digest live, every
time, or the verdict names something else. A design that describes the seal as
"hashes nothing" is describing a weaker program than the one that ships.

**Rule: the byte-to-digest binding is never ratcheted.** Every candidate below
keeps it. The ~8,000 CU it costs across thirteen records (`0005:471`) is the
price of the pattern, not a target.

### 1.2 The discriminator: a verdict earns an account, a derivation does not

The same ruling session contains the constraint that decides most of this
document:

> **ALL KEYS MUST TRANSACT.** … Target: zero `find_program_address` on the
> public hot path — every bump stored at creation or caller-supplied and
> verified by `create_program_address` (the derivation is the check).

A **derivation** — "the canonical PDA for these seeds is X" — already has a
cheap, account-free, already-ruled remedy: carry the bump, reproduce with
`create_program_address` at 1,500 CU. A ratchet account that carries only
addresses buys the difference between 1,500 CU and 0 CU per address, and pays
for it with a permanently unreclaimable rent-exempt account (`OMISSION_INDEX`
P-006).

A **verdict** — "this executable's structural validator accepted these bytes" —
has no cheap live equivalent. Its only alternative is to run the validator.

> **A ratchet carrier earns an account only when it carries a verdict.
> Addresses ride along free on an account a verdict already justified; they
> never justify one alone.**

That rule is why decision 0005 was correct to persist six raw/staging address
pairs — twelve `find_program_address` calls, itemised at ~20,000 CU (`0005:427`)
— because the account existed for the verdict already. It is also the reason to
refuse most of what follows.

### 1.3 The three staleness classes

A sealed verdict is a proposition about the past. It is safe exactly when every
mutation that could falsify it is *forced to move a byte the reader checks
anyway*. Call that byte the **staleness witness**. Three classes exist on this
route, and the tree already answers two of them:

| class | what changes | witness | refusal |
|---|---|---|---|
| **S-1 validator identity** | a Trading release cuts; the compiled validator may differ | `trading_semantic_release`, a *seed* of the seal address | the new release derives a different address, finds no account, refuses. Fail-closed by addressing, not by discipline (`0005:252-256`) |
| **S-2 substrate deployment** | a role program is upgraded under an activated release set | ProgramData's deployment slot and upgrade authority | `cached_role_deployment_observation_v1` (`crates/dclutch-registry-activation-auth-v1/src/lib.rs:468`) refuses `ReleaseSuperseded`; the Loader writes the slot on every `Upgrade` and refuses an `Upgrade` in the deployment's own slot, so slot equality *proves* the bytes have not moved (decision 0012) |
| **S-3 the sealed-over account's own state** | the account whose properties were sealed changes after sealing | none — the account is not in the frame | **see §7. This is the hard one, it is already live in the shipped seal, and it rests on one condition at `record_v1.rs:342`** |

S-1 and S-2 are the same idea in two registers: put the thing that could change
where a change makes the reader look somewhere else, or compare a byte the
mutation cannot avoid moving. S-3 has neither answer for free.

## 2. Attribution: what is actually inside the census's buckets

Everything here maps to `hot_cu_checkpoint!` regions in
`programs/dclutch-trading-sbf/src/hot_v3.rs`, so the arithmetic is checkable.

**Region A**, `"start"` (`hot_v3.rs:1946`) → `"root-product"`
(`hot_v3.rs:1978`), is the census's *Market + Direct root + product-graph record
authentication*, 109,139 CU. The two Registry CPIs are issued inside it —
`reauthenticate_top_level_root_roles_v3` (`hot_v3.rs:3423`) is reached only from
`authenticate_root_boxed_v3` (`hot_v3.rs:3363`) — so the census's 109,139 is the
region delta **net of** the 52,592 it reports as its own bucket. Region A
contains, in order: `authenticate_hot_invocation_v3`, `HotFrameV3::parse`,
`hash(family_request)`, `hash(root bytes)` against the envelope,
`authenticate_market`, `authenticate_root_boxed_v3` (CPIs +
`TradingFamilyContextV1::authenticate` + `CapabilityRootHeaderV1::decode` +
`authenticate_activated_child_programs_v3`), and
`authenticate_product_runtime_v3`.

**Region B**, `"root-product"` → `"artifacts-strategy-effect"`
(`hot_v3.rs:2263`), is the census's *sealed artifacts + execution-strategy record
+ Effect decode*, 50,761 CU. It is three `borrow_finalized_record_at` calls
(manifest, program set, config), `authenticate_capability_seal_v3`, six
`borrow_sealed_record` calls, five `sealed_token` mints, the `from_sealed`
constructors, `authenticate_strategy_from_sealed_boxed_v3`, and the two verdict
consumptions `authenticate_profile_join` and `authenticate_static_ownership`.

### 2.1 Derivation cost, measured today

`programs/dclutch-trading-sbf/program-test/tests/direct_hot_record_depth_census.rs`
reports the constant-class record depths. Run at HEAD on this tree
(`SBF_OUT_DIR` = the prebuilt `target/sbpf-solana-solana/release` ELF set;
the record rows are content-seeded and so ELF-independent by that file's own
argument — only the `capability-seal` row depends on the Trading release, and it
is already carried):

```
RECDEPTH  record           raw  staging  search_cu  carried_cu  saving_cu  status
RECDEPTH  capability-seal  255  -             1500        1500          0  CARRIED
RECDEPTH  product          252  255          7500        3000       4500  SEARCHES
RECDEPTH  result-domain    255  253          6000        3000       3000  SEARCHES
RECDEPTH  portfolio        255  255          3000        3000          0  SEARCHES
RECDEPTH  linked-basis     255  255          3000        3000          0  SEARCHES
RECDEPTH  realm            253  251         12000        3000       9000  CARRIED
RECDEPTH  exec-strategy    253  255          6000        3000       3000  SEARCHES
TOTAL if every row searched: 39000 CU; if carried: 19500 CU; difference 19500 CU,
of which 9000 is ALREADY BANKED and 10500 is still on the table
```

So in Region A, the four product-graph pairs cost **19,500 CU measured** of pure
`find_program_address`, from `authenticate_record`
(`crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs:460,465` — two searches
per record, four records). Adding the Market's own address (1–4 attempts at
`9dbbc371` per the variance census, so 1,500–6,000 CU) and the activation
cache's carried reproduction (1,500 CU) gives Region A a derivation share of
**22,500–27,000 CU, about a fifth of 109,139**.

Two caveats the variance census imposes and this document inherits. The Market
term is a *fixture artifact*: `fixture.rs:673` stages
`StateBumpsV1::UNRECORDED`, so all three Market readers take the search fallback
and the CoreState carry is dead code on the measured route (census §2). Fixing
the fixture removes that term entirely, and until it is fixed no number from that
fixture may be quoted as evidence that the CoreState carry saved anything. The
10,500 CU R-2 claims is unaffected, because it comes from
`direct_hot_record_depth_census.rs`, which derives depths from the planted
addresses rather than from execution.

**The other 82,100–86,600 CU of Region A is decode, hash and join work.** That is
where a verdict-carrying ratchet would have to bite, and it is the number this
document cannot split further without one checkpoint (§8).

### 2.2 Three decodes of one immutable account

The activation cache is `ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1` = 48 + 5×248
= **1,288 bytes**. `ActivatedExecutionReleaseSetViewV1::decode`
(`crates/dclutch-registry-contract/src/activation.rs:170`) runs
`validate_projection`, which is `release_set_projection()` (five `decode_role`
calls) plus a pairwise loop over ten role pairs decoding two roles each —
**25 `decode_role` calls per decode**, each parsing a 32-byte identity and a
216-byte `ArtifactReleaseV1`.

A top-level Direct transaction runs that decode **three times**: once inside
each of the two `RegistryInstructionV1::Reauthenticate` CPIs
(`programs/dclutch-registry-sbf/src/lib.rs:328`) and once locally in
`authenticate_activated_child_programs_v3` (`hot_v3.rs:1463`). **Seventy-five
role decodes, one account, one answer.** The continuation route runs it once
(`authenticate_accelerator_activation_v4`, `hot_v3.rs:1498`).

## 3. Candidate R-1 — the Registry reauthentication pair

**Status: recommended, first, and it is not a new seal.**

### What is re-proven per transaction, and where it was first proven

The fact is: *the deployment currently at `(program, programdata)` for role R is
the artifact release the activated release set names, and the cache is the
Registry-owned cache for this Market's selected release set.*

It was first proven by `process_activate_role`
(`programs/dclutch-registry-sbf/src/lib.rs:228`), which authenticated the
finalized release-set record and each role's artifact record and wrote the
verdict into the Registry-owned activation cache. **The ratchet carrier already
exists and already holds the verdict.** What the top-level hot route does not do
is spend it: it pays two CPIs to have the Registry spend it on its behalf.

This is VARIANCE §2's shape exactly — *carrier exists and is inert* — one layer
up.

### The carrier

`ActivatedExecutionReleaseSetViewV1` over `frame.activation_cache`, already held
to its address, owner, width and privileges by
`require_activation_cache_account_v3` (`hot_v3.rs:1383`) before any byte is read.
**No new account. No rent. No packet byte. P-006 is untouched.** Every account
the local read needs is already in the 39-slot fixed frame: `activation_cache`,
`core_program`, `core_programdata`, `trading_program`, `trading_programdata`,
`registry` (`hot_v3.rs:10606-10611`).

### The invalidation story

Identical to the CPI's, because it is the same code. `process_reauthenticate`
calls `authenticate_activated_role_in_cache_v1`
(`crates/dclutch-registry-activation-auth-v1/src/lib.rs:322`); so does every
child role program; so would Trading's top-level arm. The Registry program's own
handler says so at `programs/dclutch-registry-sbf/src/lib.rs:331-335`:

> The body below this point is shared with every role adapter that reads the
> cache directly instead of invoking this route. … the two readers must be the
> same code, not two implementations of the same rule.

Staleness class S-2, answered by decision 0012's slot pin: a substrate upgrade
moves ProgramData's deployment slot, `cached_role_deployment_observation_v1`
compares it, and a superseded release refuses `ReleaseSuperseded` rather than
executing on a verdict about bytes that have moved. Nothing about that argument
lives inside the Registry's *process*; it lives in the shared crate and in the
Loader's own refusal to redeploy in a deployment's own slot.

Staleness class S-1 does not arise: no verdict is persisted by this change.

### What it does NOT cover

- **It does not authenticate the Registry program itself.** Neither does the
  CPI. The Registry is pinned by *address*, from `authenticate_market`'s
  `state.identity.registry_program.to_bytes() != frame.registry.key.to_bytes()`
  refusal (`hot_v3.rs:10352`) — Core-owned, Core-authenticated Market state. The
  cache's bytes could only have been written by the program deployed at that
  address, because that is what account ownership means. This is the root of
  trust in both shapes, and the CPI adds nothing to it.
- It does not change what the child roles (Claims, Custody) are authenticated
  by: `authenticate_activated_child_programs_v3` already reads them from this
  same cache with no CPI.
- It does not touch the caller-supplied side. O-016 is untouched: the caller
  names no release, no program and no receipt; every identity comes from the
  authenticated Market state and the Registry-owned cache.

### CU

**52,592 CU gross, measured** — 26,296 + 26,296, identical on all 32 swept seeds
at both `ff543148` and `9dbbc371` (variance census §0, §1). It contains no
key-varying search, so it is code cost, not a draw.

**Replacement cost, arithmetic:** the top-level arm already pays one
`require_activation_cache_account_v3` and one full
`ActivatedExecutionReleaseSetViewV1::decode` (in
`authenticate_activated_child_programs_v3`). Converting folds Core and Trading
into that same decode — the continuation arm reads all four roles from one
decode — and adds two `decode_role` calls and two
`cached_role_deployment_observation_v1` observations. The observation is a slot
equality, an authority equality and eight account-property comparisons; the role
decode parses 248 bytes. Sized at **under 3,000 CU total**, so:

**Net ≈ 49,500 CU (arithmetic).** It also deletes two `Instruction` account and
data vector allocations and two return-data round trips, which the syscall audit
(`docs/evidence/RESOLUTION_RUNTIME_SYSCALL_AUDIT_2026_08_29.md:270-282`) names
and does not size.

### What it needs

This is **exactly option B of decision 0017**
(`docs/decisions/0017-cache-read-role-authentication.md:138`), which is ~~**OPEN,
ratification requested, ledger M-23**~~ **RATIFIED, BUILT AND MEASURED — see the
amendment below**. That record could not sell it because its
payoff was qualitative. It is no longer qualitative: it is 52,592 CU, measured,
invariant across 32 keys and two builds, and 3.8% of a transaction that refuses
one public trade in three thousand on the key draw alone.

> **R-1 IS BANKED, not proposed — amended 2026-08-31 (LEDGER-TRUE).** This
> candidate is spent. Decision 0017 reads `Status: **RATIFIED 2026-08-30 — A
> ratified, C refused. B BUILT AND MEASURED**` (`0017:3`), and option B landed
> in **`1da601e7`** (ancestor of `main`), measured at **−66,921 CU** (`0017:214`,
> §9 *"Option B as built (2026-08-30, lane CACHEREAD)"*) against the ~49,500 the
> arithmetic above predicts. **The estimate above is LOW by ~17,400 and is left
> standing on purpose** — §8.2 records why it was low, and a replacement-cost
> arithmetic that missed a whole retired decode is more useful visible than
> corrected away. **The ledger pointer is also stale**: this cites *"ledger
> M-23"*, but M-23 is the *reentrancy* question, which option B narrows without
> closing — B deletes the hot arm's Registry CPIs, while the residual below
> (`outer.rs::reauthenticate_role`, used by activation and close, and
> `direct_begin_retiring_v1.rs:685`) is what would finish the deletion that
> makes "no child route can execute under a Registry continuation" true by
> construction rather than by discipline. A reader arriving here for M-23 should
> read the residual paragraph, not this one.

Decision 0017's own residual applies unchanged and should land with it: the rule
that children must not CPI the Registry is enforced by deletion, not by a guard,
and Trading is the last program keeping both arms. Only the hot arm is on the
public route and only it carries this CU; option B's other two owners
(`outer.rs::reauthenticate_role`, used by activation and close, and
`direct_begin_retiring_v1.rs:685`) are the cheaper remainder of the same lane and
are what would finish the deletion.

## 4. Candidate R-2 — the five record pairs that still search

**Status: recommended, and it is ember's bump-carry ruling, not a seal.**

Five raw/staging pairs on the route still call `find_program_address`: product,
result-domain, portfolio, linked-basis
(`crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs:460,465`) and the
execution-strategy record. Their addresses are `[domain, schema, content_digest]`
under the Registry — no participant key, no ELF digest — so their depths are
constants of the protocol, which is the claim
`direct_hot_record_depth_census.rs` exists to assert.

- **Fact re-proven:** the canonical raw and staging coordinates for
  `(schema, digest)` under this Registry.
- **First proven:** at record finalization in the Registry, and again for the
  product record by the Market's founding, which had to find it to bind
  `market.identity.product_record`.
- **Carrier:** a stored bump, per ember's ruling — `StateBumpsV1` in `CoreState`
  for the product-graph rows (the product record is already a Market identity
  field), `SelectedRecordBumpsV1` in the capability root for the strategy row,
  both of which already exist and already carry bumps of exactly this class.
- **Invalidation:** none needed, and this is the point. A bump is not a verdict.
  A wrong byte reproduces a different address and refuses at the equality — the
  derivation *is* the check (`hot_v3.rs:10370-10376`,
  `crates/dclutch-registry-contract/src/activation.rs:44-46`). There is no
  staleness window because there is nothing asserted.
- **Does not cover:** nothing about the record's contents, ownership, width,
  rent or digest changes; all of those stay live.
- **CU: 10,500, measured today** (the census's "still on the table" figure).

**Why this is in a trust-ratchet document at all:** because the seal-class
alternative — persisting the ten *addresses* rather than the five bumps — is
worth 25,500 CU instead of 10,500, and it must be refused. The extra 15,000 CU
costs a new account, its rent, and a second class of P-006-stranded state. Under
§1.2's rule an address does not earn an account. **Refused; take the 10,500.**

> **R-2 IS STILL OPEN — but it is no longer speculative. Verified at HEAD
> 2026-08-31 (LEDGER-TRUE).** Two things moved under it, in opposite directions.
>
> **The 10,500 is still on the table, unclaimed.** All five raw/staging pairs
> still call `find_program_address` — `crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs:460`
> (raw) and `:465` (staging), exactly the lines this row cites, unchanged. No
> bump reached `StateBumpsV1` or `SelectedRecordBumpsV1` for these records.
>
> **But this row's central argument has since been demonstrated in production,
> on a different site, by lane ALLKEYS.** The *"a bump is not a verdict"*
> invalidation story is no longer an argument in a design document — it ships
> as the Hot **root** bump hint: `hot_bump_hint_v1`
> (`crates/dclutch-capability-program-contract/src/hot_v3.rs:324`) and
> `HotBumpHintsV1`, consumed at
> `programs/dclutch-trading-sbf/src/dispatch.rs:370`, landed across `c5f5099c`,
> `f27ecc07`, `62f2a727` and `f346ba81` (all ancestors of `main`). The
> adversarial case is named after this row's own reasoning —
> `dispatch.rs:806`, `a_wrong_root_bump_hint_reproduces_another_address_and_refuses`.
> That is precisely *"a wrong byte reproduces a different address and refuses at
> the equality — the derivation IS the check"*, executed rather than asserted.
>
> **Two consequences for whoever takes R-2.** (1) The O-016 objection is
> pre-answered: a caller-supplied bump hint is admitted tree-wide today, because
> it enters as a *hint* whose only effect is to be re-derived and compared —
> caller input that becomes authority by inclusion is what O-016 forbids, and
> this is not that. (2) There is now a **shipped template** to copy rather than a
> pattern to design, so the remaining work is mechanical: carry the hint, derive,
> compare, refuse. R-2's cost estimate should be re-read as an implementation
> ticket, not a proposal.

## 5. Candidate R-3 — the product-graph closure verdict

**Status: sized as a bracket, refused for now on P-006, and it is the only
candidate that would be a genuine second seal.**

### The fact

`authenticate_product_runtime_v3`
(`crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs:363`) and
`authenticate_product_basis_v3` (`:391`) run, on every transaction, over four
immutable content-addressed records:

`ProductRecordV2::decode`, `ResultDomainV2::decode`, `PortfolioV2::decode`,
`ProductBasisV3::decode`, `admit_authenticated_views_v2`, a `hashv` over
`semantic_basis_preimage_v3` to derive `semantic_basis_id`, and ten equality
joins across the four (`:313-320`, `:429-434`).

Every one of those is a total deterministic function of the four records'
bytes. None reads a sysvar, a clock, the request, or the Market's mutable state.
This is decision 0005's predicate verbatim, one graph over.

The graph root is anchored harder than the descriptor closure is:
`product_record` is one of the nine seeds of `MarketCoreStateSeedsV2`
(`crates/dclutch-market-core-codec/src/physical.rs:637,653`), so the Market's own
address depends on it. It cannot change without the Market being a different
account, and `authenticate_market` reproduces that address every transaction
(`hot_v3.rs:10355`). The rest of the graph hangs off it by content.

### The carrier, if it is ever built

A second Trading-owned, write-once, content-addressed seal:

```text
seal = find_program_address(
    [ PRODUCT_GRAPH_SEAL_PDA_DOMAIN_V1,
      product_record_digest,     // 32
      linked_basis_digest,       // 32
      trading_semantic_release,  // 32
      registry_program ],        // 32
    trading_program)
```

Four rows — product, result-domain, portfolio, linked-basis — each carrying
schema, content digest, canonical raw and staging addresses and exact width;
verdict bits for the admission projection and the basis joins; and the derived
`semantic_basis_id`, `coordinate_domain_id`, `result_unit_id`, `outcome_count`.

**`linked_basis_digest` must be a seed and not a payload, and this is the O-016
line.** `ProductRecordV2` deliberately does not pin a linked-basis raw digest
(`:357-361`); the basis is derived from the account the *caller* supplied and
then joined. That makes its identity caller-chosen. A seal that carried it as a
row would let a seal minted for one basis be presented with another; a seal that
carries it as a *seed* refuses by finding nothing at the derived address. This
is the same argument decision 0005 made for putting the Registry program in the
seeds (`0005:143-152`).

`trading_semantic_release` must also stay a seed, for decision 0005's reason
(`0005:495-502`): the validators live in `dclutch-product-runtime-v2-*` crates
but are *compiled into Trading*, and the Trading semantic release is the only
in-band identity of the compiled validator. The lifting plan is the same one —
an identity emitted from the validators rather than asserted beside them — and
it is not a reason to start unsound.

**Key cardinality is O(products × bases × releases), not O(markets).** Many
Markets share a product record. That is strictly better than per-Market and it
is why the content-addressed form is the only one worth considering.

### Why it is refused today

**P-006 doubles.** `OMISSION_INDEX.md` P-006 records that a capability seal is
write-once, closed by nothing, and — because `trading_semantic_release` is a
seed — *"every Trading release therefore permanently strands the rent of every
seal written under its predecessor, across all descriptors × actions. The account
class only grows, and it grows with the release cadence rather than the Market
count."* `CloseSeal` has zero occurrences in any `.rs` or `.lean` in the tree.

A second seal class adds a second stranded population growing on the same
cadence — at the capability seal's layout shape (152-byte header, 136-byte rows)
four rows is **696 bytes**, ≈0.0057 SOL, scaled from P-006's measured 968-byte
seal at ≈0.00763 SOL — before the first close route exists. That is a decision
about permanent state and it should not be taken as a side effect of a CU lane.

**The frame.** The seal adds one account (39 → 40) and one ALT-routed key
(one wire byte). Decision 0005 recorded the canonical continuation packet at
1,225 B of 1,232 (`0005:335`) and the continuation route was 1,206 B at
`CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md:95`. One byte fits; two
consecutive account additions would need the wire re-measured first. It would,
however, let the four product-graph *staging* slots become aliases of their raws
under `SEALED_EXECUTION_FIXED_ALIASES_V3`'s existing pattern (`hot_v3.rs:10631`),
removing four unique transaction locks without changing the account count.

### CU

**Unmeasured, and strictly less than 82,000–87,000 CU.** Region A net of its
22,500–27,000 CU of derivation is 82,100–86,600 CU, and R-3's take is a *strict
subset* of that residue: the rest of it is `HotFrameV3::parse`,
`authenticate_hot_invocation_v3`, two body hashes, `CoreState::decode` and its
canonical re-encode, `TradingFamilyContextV1::authenticate`,
`CapabilityRootHeaderV1::decode` and the activation-cache decode. The
product-graph share is not separable from the published table, and this document
declines to guess at it.

The corroborating figure is decision 0005's own step table, which measured
*"root + Product runtime"* at **98,519 CU** (`0005:32`) on the Direct Profile14
bundle — the same region, a different build.

**One `hot_cu_checkpoint!("root")` between `authenticate_root_boxed_v3` and
`authenticate_product_runtime_boxed_v3` (`hot_v3.rs:1976`) splits this exactly.**
That is the measurement that decides whether R-3 is worth a permanent account
class, and it is one line.

## 6. Refusals

### R-4 — carry the manifest, program-set and config *addresses* in the root

Region B still pays seven derivations: three `borrow_finalized_record_at` pairs
at 1,500 CU each side, plus the seal's own reproduced address. **10,500 CU
measured-by-construction** (`create_program_address` is 1,500 CU;
`direct_hot_record_depth_census.rs:107-112`), of which 9,000 is the three record
pairs.

**Refused.** The carrier would have to be the capability root's *immutable*
232-byte `CapabilityRootHeaderV1`, which decision 0005 kept deliberately
untouched (`0005:342-348`). Six addresses is 192 bytes, nearly doubling a header
that every existing Market's root already carries at its current width. That is a
generation-wide migration for 9,000 CU. If a header migration happens for another
reason, this rides along; it does not justify one.

### R-5 — seal the Market state's canonical re-encode

`authenticate_market` (`hot_v3.rs:10333`) decodes `CoreState`, re-encodes it, and
compares the result byte-for-byte against the account (`:10345-10349`). That is a
per-transaction re-proof that Core wrote canonical bytes — a fact Core
established at write time.

**Refused, and the reason generalises.** The Market state is mutated by every
trade. A witness pinning "these Market bytes are canonical" is falsified by the
next write, so its staleness rate on the route that would consume it is **1**. A
ratchet whose verdict is always stale is not a ratchet; it is an extra account
and an extra refusal.

**This is the boundary the ruling draws.** "Trust ratchets forward as state
mutates" is a claim about facts that *do not* mutate — write-once,
content-addressed, program-owned. The Market state is the counterexample, and
naming it keeps the pattern from being applied where it degrades into a cache
with an invalidation problem.

The re-encode has a real remedy and it is not a ratchet: a decoder that refuses
non-canonical bytes by construction makes the re-encode unnecessary. That is a
validator change, owned by whoever owns `dclutch-market-core-codec`, and it is
recorded here as a handoff rather than designed.

### R-6 — seal `require_common_projection_bindings_v3`

The join over (selected config, product record, product id, semantic basis,
linked basis) at `hot_v3.rs:2102` is a fact about Market identity fields that are
immutable within a generation, so it *is* sealable. **Refused on value:**
decision 0005 measured the whole *"config borrow + common projection bindings"*
step at **4,413 CU** (`0005:37`). No account is worth 4,413 CU under §1.2.

### R-7 — request-scoped authorities

The caller-authority PDAs — `hot_v3.rs:1746`, `child_authority_v4.rs:65`,
`claims_composition_v3.rs:162` — are seeded by a request digest or packet digest
that is a new value in every transaction. **Structurally unratchetable**, as the
variance census already classifies them (class (c), irreducible). Recorded so no
later lane spends time on them.

## 7. The hard case: S-3, and what the shipped seal already assumes

The seal removes the *staging cursor* from the frame. `borrow_sealed_record`
requires the frame's staging slot to alias the raw account
(`staging.key != raw.key` refuses, `seal.rs:477`) and requires the seal's own row
to record two *distinct* addresses (`seal.rs:476`). The real staging cursor
account is not present, and its current vacancy is not checked. The hot path
says so plainly at the point of use (`hot_v3.rs:2028-2029`):

> the seal is the durable proof that the real staging cursor was vacant when
> this exact raw body was admitted.

So the shipped seal carries one S-3 proposition: **"at seal time, the canonical
staging cursor for this `(schema, digest)` was vacant and System-owned."**
Everything else the seal consumes is either an address (S-3-free: an address is a
pure function of seeds and program id) or a verdict about bytes that are
re-pinned live.

That proposition is sound exactly if a finalized record's staging cursor cannot
be made non-vacant again, and its raw body cannot be rewritten. If it can, the
seal is a durable claim about a state the chain has since left, and no seed and
no witness in the current design would notice — because the account is not in
the frame to be looked at.

### 7.1 Why it holds today, exactly

The Registry has four record routes, all in
`programs/dclutch-registry-sbf/src/record_v1.rs`: `Begin` (`:243`), `AppendPage`
(`:408`), `Finalize` (`:458`), `Abort` (`:531`). It is not true that no close
route exists — **`Abort` genuinely closes both PDAs** (`:598-599`), zeroing
lamports, resizing to nothing and assigning back to System. The property is not
"there is no destructor"; it is **finalization is the point of no return**, and
it is enforced by two refusals that are worth naming separately because only one
of them is load-bearing.

`Append`, `Finalize` and `Abort` all pass `require_live_record_accounts`
(`record_v1.rs:792-807`), which requires the cursor to be Registry-owned,
`STAGING_CURSOR_BYTES_V1` wide and lamport-bearing. `Finalize` destroys exactly
that cursor (`close_full_to_wallet`, `:495` and `:809`), so after finalization
all three routes refuse. `StagingStatusV1` has exactly one variant, `Building`
(`crates/dclutch-record-contract/src/lib.rs:788-791`) — there is no encodable
"finalized" cursor state, so a finalized record simply has no cursor. `Finalize`
additionally requires the raw account **non-writable**
(`require_privilege(frame.raw, false, false, false)`, `record_v1.rs:159`), and
`AppendPage` is the only route that ever mutates raw bytes.

**The load-bearing refusal is in `Begin`, and it is about the raw account, not
the cursor.** After finalization the cursor *is* vacant, so `Begin`'s cursor
check would pass. What stops a re-`Begin` at the same canonical key is
`authenticate_begin` (`:333`) calling `require_prefunded_vacant(frame.raw)`
(`record_v1.rs:342`), where `is_prefunded_vacant` (`:991-995`) demands
System-owned, non-executable and empty. A finalized raw record is Registry-owned
with `exact_length` bytes, so it fails with `RegistryError::Record`. The cursor's
own `require_prefunded_vacant` on the next line (`:343`) is satisfied by a
finalized record and refuses nothing here.

The mint side inherits this correctly: `process_capability_seal_v1` reads its six
records through `borrow_finalized_record`, whose `borrow_record_against`
(`hot_v3.rs:10570-10575`) requires the real staging cursor to be System-owned and
zero-length. So a seal cannot be minted against a mid-build record, and once
minted it names a record that can never leave the finalized state. The chain is:

> seal exists ⟹ the cursor was vacant at mint ⟹ the record was finalized ⟹
> (no route returns a finalized raw account to `is_prefunded_vacant`) the record
> is finalized now.

### 7.2 The exact shape of the failure, if that one refusal ever moves

Suppose a record-reclamation route is added — and P-006's rent argument invites
one, because the same "prepaid publication cost, closed by nothing" reasoning
applies to record accounts as to seals. Suppose it returns a finalized raw
account to System-owned and empty. Then a re-`Begin` at the same
`(schema, digest)` becomes legal, and a live cursor reappears at the sealed
staging coordinate.

The dangerous state is not the empty one and not the finished one. During
`Begin`, the raw account is zeros and `hash(data) != digest`, so
`borrow_sealed_record` refuses. After a re-`Finalize`, the bytes hash to the same
digest and the cursor is gone again, so the seal is correct. **The window is
after the last `AppendPage` and before `Finalize`:** the raw account holds
exactly the bytes that hash to `digest`, and the cursor is live. At that instant
`borrow_finalized_record` refuses on cursor vacancy and `borrow_sealed_record`
**accepts**, because the cursor is not in its frame.

Every other condition `borrow_sealed_record` checks is satisfied in that window:
`Begin` allocates the exact width and prefunds rent exemption, the Registry owns
the raw account throughout, the bytes hash to the sealed digest, and the caller
supplies the account read-only in a transaction of its own.

**Be precise about the harm, because overstating it would make this section
untrustworthy.** The bytes in that window are the sealed bytes — the digest
check forces that — so the execution's own arithmetic is not wrong. What breaks
is the invariant that everything a hot action executes against is
Registry-*finalized*: a mid-build record can still be `Abort`ed or appended to,
so its bytes are not yet permanent, and permanence is what makes a content digest
an identity rather than a snapshot. The record contract states the same thing
from the other side — *"the raw account's PDA and apparent payload alone never
assert finality"* (`crates/dclutch-record-contract/src/lib.rs:1556-1560`) — which
is exactly why cursor vacancy is the finality proof and why removing the cursor
from the frame is the thing that has to be argued.

That is the whole of S-3 on this route, stated as a reachable state rather than a
worry. Today it is unreachable because nothing returns a finalized raw account to
`is_prefunded_vacant`; the condition at `record_v1.rs:342` is the one that would
have to be reasoned about the day a reclamation route is written.

### 7.3 The requirement

> Any future Registry route that vacates, re-stages, rewrites or closes a
> **finalized** record must be refused at design time, or must invalidate every
> Trading seal naming that record. There is no third option, because the seal
> does not carry the account it would need in order to notice.

This is decision 0017 §3's subtractive enforcement in another place, with the
same weakness: nothing refuses the route, there is simply no such route to write
against. It should become a tripwire — an adversarial test that mints a seal,
attempts the reclaim-and-restage sequence, and asserts the refusal at
`authenticate_begin` — so the seal's soundness stops resting on a condition
nobody has been told is load-bearing.

**The tripwire LANDED 2026-08-31 (CLOSESEAL).**
`programs/dclutch-registry-sbf/src/record_v1.rs`,
`record_v1::tests::finalization_is_the_point_of_no_return_and_the_raw_account_is_what_enforces_it`.
It is a one-variable control rather than a restatement of `is_prefunded_vacant`:
the same `begin_fixture`, the same `BeginRecordV1`, the same vacant cursor, and
only the raw account moves the answer — vacant admits, finalized refuses, and
Registry-owned-but-emptied (which is what a careless reclamation route would
leave) also refuses. It asserts §7.1's split explicitly, that the *cursor*'s
`require_prefunded_vacant` is SATISFIED by a finalized record and refuses
nothing, so the reader cannot come away thinking the cursor check is what holds
this up. Its doc comment carries the finality-window argument and names
`borrow_sealed_record`, so the lane that reddens it is told what it has to
re-argue rather than left to delete an assertion.

It does not mint a seal, and that half of the ask is deliberately not faked: a
Registry unit test cannot reach Trading's seal writer, and a program-test that
staged the reclaim-and-restage sequence would need a reclamation route that does
not exist to stage it with. The tripwire fires on the condition, which is the
thing a future route must break.

R-3 inherits this proposition unchanged and multiplies it by four records. That
is a second reason its charter should follow P-006's close-route ruling rather
than precede it: the two questions are one question about who may un-finalize
state that other accounts have already recorded verdicts about.

One boundary the argument does not cover: it is about the shipped Registry ELF. A
Loader upgrade of the Registry program itself introduces code that is not a route
in the program as written, and is answered by S-2's slot pin at the release-set
level, not here.

## 8. What would settle the open numbers

1. **One checkpoint.** `hot_cu_checkpoint!("root")` at `hot_v3.rs:1976`, between
   `authenticate_root_boxed_v3` and `authenticate_product_runtime_boxed_v3`.
   Splits the 109,139 CU bucket into root authentication and product-graph
   authentication and turns R-3's bracket into a number. The profiled
   build's 2.10% instrumentation load and extended-heap arm are already
   characterised by the variance census §5, so the delta is attributable.
2. **R-1 measured on landing. ANSWERED 2026-08-30 (lane CACHEREAD).** The
   post-conversion figure at the floor statistic this asked for: **1,319,672 ->
   1,252,751 CU, a measured 66,921**, over 32 seeds. The ~49,500 estimate was
   LOW by about 17,400, and the reason is instructive rather than a rounding
   error: this document's replacement-cost arithmetic sized the two CPIs and the
   two role decodes that replace them, and did not notice that folding the roles
   into the existing decode also retires the SEPARATE decode
   `authenticate_activated_child_programs_v3` was paying. §2 of this document had
   already counted that decode -- "seventy-five role decodes, one account, one
   answer" -- but §R-1's arithmetic did not credit removing it. It is worth about
   14,300 CU. See `docs/decisions/0017-cache-read-role-authentication.md` §9.

## 9. Rulings requested

1. **Ratify decision 0017 and charter its option B**, now that its payoff is
   52,592 CU measured rather than a qualitative label. This is the whole of R-1
   and it needs no new state.
2. **P-006 before R-3.** Do we accept a second permanently-stranded, release-
   cadence-growing seal class before the first one has a close route? This
   document recommends no. The seal beneficiary question P-006 raises is
   sharpened by R-3, not answered: a product-graph seal is not per-Market either
   — it is per `(product, basis, release)` — so no Market's `FundingStateV1` may
   receive its refund without one Market paying for every other's, which is the
   same reasoning that put the capability seal outside custody in the first
   place. A plausible beneficiary is the last closer, permissionlessly, with the
   write-once discipline preserved by refusing to re-seal a closed address into a
   different verdict; that is a design, not a line, and it is P-006's to write.

**ALL THREE ANSWERED — verified at HEAD 2026-08-31 (LEDGER-TRUE).** The section
is kept as asked, with each request's disposition appended, because two of the
three were answered by lanes that never came back to this file.

1. **GRANTED, and BUILT.** Decision 0017 is `Status: **RATIFIED 2026-08-30 — A
   ratified, C refused. B BUILT AND MEASURED**` (`0017:3`); the ratification is
   the decision packet's §3 (`decisions/DECISION_PACKET_2026_08_30.md:55-62`),
   which chartered B on exactly this document's 52,592 CU measurement. **B
   landed in `1da601e7`** (*"merge: CACHEREAD — 0017 option B, measured at
   −66,921 CU, with the wall's first demonstrated red"*), an ancestor of `main`.
   The realised figure is **−66,921** (`0017:214`), not the ~49,500 this
   document's §3 arithmetic predicted — §8.2 already carries that correction and
   the reason for it (the folded decode `authenticate_activated_child_programs_v3`
   was retired too, worth ~14,300 CU, which §R-1's arithmetic did not credit).
   R-1 is therefore no longer a candidate; it is banked.
2. **ANSWERED IN THE ORDER THIS DOCUMENT ASKED FOR.** P-006 did come before R-3:
   it is `CLOSED 2026-08-31` (`OMISSION_INDEX.md` P-006), and the beneficiary
   ruling is the one this item called plausible — **the closer, capped**, the
   funded-crank pattern, no Market's funding receiving it, burn rejected
   (`WAVE.md`, *"Rulings — 2026-08-31, ember's full-autonomy directive"*). The
   write-once discipline is preserved the way this item proposed, and by a
   sharper mechanism than "refusing to re-seal": the close only admits a cache
   whose semantic release **differs** from the seal's fourth seed
   (`0x400A CloseSealLiveRelease`), and the writer derives its address from that
   same live release — so a closed seal is an address the live executable cannot
   reach, not one it declines to write. **R-3's charter is unblocked**, and it
   inherits the profile gate as well as the beneficiary rule.
3. **ADOPTED, and the test exists.** See §7.3, which CLOSESEAL extended in place.
   Verified at HEAD: the guard is still the single condition this section named,
   `require_prefunded_vacant(frame.raw)?;` at
   `programs/dclutch-registry-sbf/src/record_v1.rs:342` — unchanged, and now
   sitting next to `require_prefunded_vacant(frame.cursor)?;` at `:343`, which
   §7.3 is careful to say refuses nothing here. The adversarial test is
   `record_v1::tests::finalization_is_the_point_of_no_return_and_the_raw_account_is_what_enforces_it`
   (`record_v1.rs:1720` at HEAD). The half of the ask that was **not** delivered
   is named rather than quietly dropped: it does not mint a seal, because a
   Registry unit test cannot reach Trading's seal writer and the reclaim-and-
   restage sequence would need a reclamation route that does not exist to stage
   it with. It fires on the condition instead, which is the thing a future route
   must break.
3. **The S-3 tripwire.** Adopt §7.3 as a standing constraint on the Registry's
   record routes. The shipped capability seal's soundness currently rests on one
   condition — `require_prefunded_vacant(frame.raw)` at `record_v1.rs:342` —
   that nobody has been told is load-bearing for anything but `Begin`'s own
   hygiene. It should carry a comment naming the seal, and an adversarial test
   that attempts the reclaim-and-restage sequence against a minted seal.

## 10. What this document does not claim

- It does not claim the W2 compute gate passes. R-1 and R-2 together are ~60,000
  CU against a route whose key-independent floor is 1,321,742 CU and whose
  protocol ceiling is 1,400,000. What they buy is slack: the ceiling question
  goes from 78,258 CU above `C0` to about 138,000. Both candidates lower `C0`;
  neither changes the ten key-varying site count, which is CARRY's list, not this
  one. The resulting refusal probability is the census §4 calculation to redo
  against the new `C0`, not a number this document will invent.
- It does not close the fee-bearing two-Custody shape the variance census §3
  sizes at 1.49–1.52M and which nobody owns. That is the largest thing on the
  route and none of these candidates touch it.
- It does not weaken any live refusal. Every candidate keeps the byte-to-digest
  binding, the owner and privilege conjunction, the rent exemption and the exact
  width on every record, sealed or not.
- It does not touch caller-supplied data. O-016 stands: no candidate here admits
  a caller-named program identity, effect plan, semantic ID or account
  projection. Where a caller-chosen identity enters a proposed key — the linked
  basis in R-3 — it enters as a *seed*, so a substitution refuses by finding
  nothing rather than by being checked.
