# Decision 0005: validated-artifact evidence is content-addressed, not per-Market

Status: accepted on 2026-08-26 as the resolution of the compute half of the W2
common-Hot gate. This is an architecture and authority decision, not release or
deployment evidence. It does not change what any artifact means, does not change
what a capability root is, and does not weaken any refusal the hot path makes
today. It is explicitly **not** a claim that it makes the W2 gate pass: the
measurement below shows it does not, and says why.

The lane was chartered to design a *per-Market* authentication cache written by
the Trading activation act. The measurement and the ground truth moved the
answer: the fact being cached is a fact about content, not about a Market, and
the activation act does not authenticate the artifacts in question at all. Both
departures are argued below and the chartered shape is kept as a named rejected
alternative rather than quietly dropped.

## Context

### What the hot path spends

Measured on the canonical Direct Profile14 bundle at HEAD, with
`hot-cu-profile` checkpoints, on the real 32,768-byte heap, 3,000,000-CU
diagnostic budget. The bundle refuses with `Custom(3)` inside the lifecycle
preplan on a Direct emitter defect owned by another lane; everything before that
refusal is exact. (`Custom(3)` was `TradingSbfError::Content`. Decision 0007
later namespaced every refusal code and it is `0x4003` now; the number here is
left as it was measured.)

| step | CU | heap |
| --- | ---: | ---: |
| entry -> start | 11,884 | 8,305 |
| root + Product runtime | 98,519 | +5,472 |
| manifest borrow | 4,177 | +0 |
| program-set borrow + decode + `select_entry` | 18,823 | +527 |
| descriptor borrow | 7,223 | +0 |
| descriptor decode (`CapabilityProgramV4`) | 5,471 | +583 |
| config borrow + common projection bindings | 4,413 | +0 |
| lifecycle borrow + `StateLifecyclePolicyV5::decode` | 7,273 | +0 |
| account-profile borrow + `AccountProfileV2::decode` | 105,252 | +0 |
| `validate_account_profile_join` | 82,337 | +0 |
| request-profile borrow + decode | 105,112 | +0 |
| strategy authentication | 15,497 | +1,731 |
| transition borrow + `TransitionProgramV3::decode` | 24,111 | +0 |
| effect borrow + `decode_selected_effect_v4` | 263,344 | +0 |
| runtime observations | 90,030 | +7,678 |
| `require_geometry` | 1,587 | +0 |
| `authenticate_current_rent_quotes_v5` | 740 | +23 |
| `project_account_and_request_registers_v3` | 303,440 | +5,006 |
| `require_static_register_ownership_v5` | 66,479 | +234 |

One `borrow_finalized_record` costs about 4,200 CU: two
`Pubkey::find_program_address` calls, one SHA-256 of the record body, and the
owner/rent/privilege/staging conjunction. The manifest row is a pure borrow with
no decode, which is what fixes that constant.

Subtracting one borrow from each row leaves the *structural validation* of the
artifacts: about 605,000 CU inside the artifacts phase, plus 66,479 CU in
`require_static_register_ownership_v5`, plus 82,337 CU for the lifecycle policy's
join to the account profile. Every one of those is a pure function of immutable,
content-addressed bytes. None of it depends on the request, the accounts, the
slot, or the Market's state. **It is recomputed identically on every execution.**

### What is not repeated work

`project_account_and_request_registers_v3` (303,440 CU) walks real account
observations; `require_lifecycle_effect_bindings_v4` (131,641 CU, measured by the
W2c lane) folds resolved effect writes against planned bindings using runtime
register values; the lifecycle preplan derives real PDAs. These are genuinely
per-execution and this decision does not touch them.

### The heap is a separate problem and this decision does not solve it

Accumulated heap at the child-route preflight is 61,889 bytes against a
32,768-byte limit. The artifacts phase contributes **2,850** of those bytes. The
rest is the boxed hot frame (8,305), root and Product runtime (5,472), the
92-coordinate observation bank (7,678), the projection register banks (5,006),
and the preplan/effect/child phases (about 32,000). Removing every identical
validation removes about 2,000 bytes.

Recorded here so no later reader mistakes this decision for a heap fix: the
61,889 figure is *total-ever-allocated*, because the default SBF allocator's
`dealloc` is a no-op and `programs/dclutch-trading-sbf` installs no
`#[global_allocator]` — its `custom-heap` Cargo feature is declared and
unimplemented. Peak live heap is far below the limit. The heap half of the gate
is an allocator/arena decision, not a caching decision, and it is still open.

### Ground truth about the activation act

`programs/dclutch-trading-sbf/src/outer.rs::process_activation` authenticates a
`CapabilityProgramV1` under `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1`, an
`AccountProfileV1`, and an effect `ProgramV2`.

`programs/dclutch-trading-sbf/src/hot_v3.rs` authenticates a
`CapabilityProgramSetV2` under `CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2`,
selects a `CapabilityProgramV4` under `PROGRAM_SCHEMA_ID_V4`, and then an
`AccountProfileV2`, a `StateLifecyclePolicyV5`, a request profile V3/V4, a
`TransitionProgramV3` and an effect `ProgramV4`.

**Activation has never authenticated a single artifact the hot path executes.**
It cannot write evidence about a conjunction it does not evaluate, and making it
evaluate that conjunction is a separate convergence of the whole activation
route onto the V4 generation, not a line of extra work inside it.

Two further facts foreclose an activation-time write even after that
convergence. `CapabilityProgramSetV2` admits up to 32 entries
(`CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2`), each naming its own descriptor and
therefore its own artifact closure; at the measured ~605,000 CU per closure,
sealing all of a Market's actions inside one activation transaction is
arithmetically impossible past the second entry. And a Market's activation
already carries 139 account references and five CPIs.

### What is actually being cached

The predicate is

> the byte string whose SHA-256 is `D`, read under schema `S`, is accepted by
> this Trading interpreter release's structural validator

together with the pairwise and n-ary joins among a descriptor's named artifacts.
`D` determines the bytes. `S` and the interpreter release determine the
validator. **The Market is not a free variable in that predicate.** Persisting
one copy of it per Market would store M copies of one fact, which is exactly what
`AGENTS.md` forbids: *one semantic owner per persisted fact*.

## Decision

**Validated-artifact evidence is persisted once per `(descriptor schema,
descriptor content digest, action selector, Trading interpreter semantic
release, Registry program)` in one Trading-owned, content-addressed, write-once
seal account. It is never persisted per Market, and no hot action may trust a
seal written for a different interpreter release.**

```text
seal = find_program_address(
    [ CAPABILITY_SEAL_PDA_DOMAIN_V1,
      descriptor_schema,        // 32
      descriptor_digest,        // 32
      action_selector_le,       // 4
      trading_semantic_release, // 32
      registry_program ],       // 32
    trading_program)
```

The Registry program is the fifth coordinate because the rows persist the
canonical raw-record and staging addresses, and those are PDAs *of a particular
Registry*. Making it a seed rather than a body field means a Market whose
selected Registry differs derives a different address and can seal there,
instead of finding one seal it must refuse forever at an address that is
write-once. The Registry a hot action compares against is the one its own
authenticated Market state names, which is the same Registry every record in the
frame is already required to be owned by.

`SealedDescriptorClosureV1` holds, in canonical order and fixed arity: the four
key coordinates repeated in the body; and one row per artifact the descriptor
names — lifecycle policy, account profile, request profile, transition program,
effect program — carrying that artifact's `schema`, `content_digest`, canonical
raw-record address, canonical staging-cursor address, and exact record width. The
descriptor's own row is row zero. Verdict bits record the joins that were proved:
the lifecycle-to-account-profile join and the static register-ownership
conjunction over `(account profile, policy, action, request profile, transition)`.

The seal is written by exactly one act: `SealDescriptorClosure`, a new Trading
instruction that performs **the identical validation the hot path performs
today**, over the identical bytes, and persists nothing but its verdict and the
addresses it derived. There is no second implementation of any validator; the
seal writer and the hot reader call the same functions in the same crate.

### What a hot action still validates live, exactly

Nothing that is checked today stops being checked. The complete live conjunction
after this decision is:

1. the instructions sysvar, the current top-level instruction, and either the
   direct-invocation identity or the full Registry-continuation admission join
   (`authenticate_hot_invocation_v3`, unchanged);
2. the whole fixed frame's privileges, executability, program identities and
   pairwise distinctness (`HotFrameV3::parse`, unchanged);
3. the root prestate digest against the envelope, byte for byte (unchanged);
4. the Market account: Core ownership, exact `STATE_BYTES`, canonical re-encode,
   `market_id`, selected release set, **Registry program**, generation, and the
   `MarketCoreStateSeedsV2` PDA (unchanged);
5. the capability root: Trading ownership, exact composite width, the immutable
   `CapabilityRootHeaderV1`, its `CapabilityRootSeedsV1` PDA, and the
   Market/generation/release-set join (unchanged);
6. `Reauthenticate(Core)` and `Reauthenticate(Trading)` against the Market's
   selected activation cache, with the return-data producer, role, release-set
   and program echoes checked (unchanged) — and the Trading receipt's
   **semantic release is now additionally a seed of the seal**, so a Trading
   upgrade invalidates every seal by construction;
7. the Product runtime: Registry finality, schemas, content digests, Product
   record, portfolio, linked semantic basis, outcome count (unchanged);
8. for **every** record, including every sealed one: `!is_signer`,
   `!is_writable`, `!executable`, `owner == registry`, rent exemption for its
   exact width, a vacant System-owned staging cursor, and
   **`hash(bytes) == content_digest`** — the byte-to-digest binding is
   recomputed live on every execution and is never taken from the seal;
9. the manifest record, the selected entry, the program set, `select_entry`
   against the actual family request, the selected action, the config record and
   `require_common_projection_bindings_v3` (unchanged, unsealed);
10. the strategy record and its transition schema/program join (unchanged,
    unsealed);
11. the request itself: dynamic span widths, `require_geometry`, request shape,
    tail-count agreement, borrowed-witness coverage, native signature evidence;
12. every account observation, every projection, the lifecycle preplan and
    replan, rent quotes, credits, balances, canonical lifecycle PDAs, effect
    projection, local preflight, child-route preflight, replay revisions, child
    receipts, and commit-last ordering (all unchanged);
13. the seal account itself: `owner == this program`, `!is_signer`,
    `!is_writable`, `!executable`, rent exemption for its exact width, the exact
    canonical PDA under the five seed coordinates above, magic, schema version,
    artifact profile, canonical reserved bytes, and that every row's `schema` and
    `content_digest` equal the identities the *authenticated descriptor* names —
    not the identities the caller supplies.

### What a hot action may consume from the seal, exactly

Only two things, and only for artifacts whose `hash(bytes) == content_digest` has
just been recomputed live:

- **the canonical raw-record and staging-cursor addresses**, replacing two
  `find_program_address` derivations per sealed record with a 32-byte comparison
  against an address derived once, from the same seeds, under the same Registry
  program that the Market's own authenticated state names; and
- **the structural-validation verdict**, replacing the `decode` sweep with a
  borrowed view constructed from the same bytes by the same cheap header parse,
  and replacing `validate_account_profile_join` and
  `require_static_register_ownership_v5` with their recorded results.

### Why that is not a weakening

The argument has exactly three steps and no gaps.

1. **The bytes are pinned live.** Step 8 above recomputes `hash(bytes)` and
   compares it with the identity the authenticated descriptor names. SHA-256 is
   collision-resistant, so a successful comparison means the borrowed bytes *are*
   the content-addressed artifact — the same conclusion the current code reaches,
   reached the same way, at the same cost. The account is required non-writable
   for the whole execution, so no CPI can change it under the borrow. Nothing in
   this decision rests on the Registry deployment being immutable, on the
   staging cursor's absence, or on any property of the account other than its
   contents, which are proved.
2. **The predicate is a function of the bytes and the validator.** Every sealed
   check — `AccountProfileV2::decode`, request-profile decode,
   `TransitionProgramV3::decode`, `EffectProgramV4::decode`,
   `StateLifecyclePolicyV5::decode_selected`, `validate_account_profile_join`,
   `require_static_register_ownership_v5` — is a total, deterministic function of
   the artifact bytes (and, for the last one, the action selector, which is a
   seed of the seal). It reads no account, no sysvar, no clock and no request.
   Therefore "it accepted these bytes once" and "it accepts these bytes now" are
   the same proposition, provided the validator has not changed.
3. **The validator cannot have changed.** The seal's fourth seed is the Trading
   interpreter's `semantic_release_id`, taken from the Registry role receipt this
   very invocation authenticated. A different Trading release derives a different
   seal address and finds no account there. This is fail-closed and requires no
   developer discipline: it is not possible to ship a changed validator that
   silently honours an old verdict.

The seal is therefore memoisation of a pure function under a key that includes
the function's own identity, over an input pinned by its own hash. It is not a
trusted assertion by a third party. The writer is this Program: the account is a
PDA it owns, no other program can allocate or assign it, and the only code path
that writes it runs the same validators.

The writing act is **permissionless**, and that is a property, not a concession.
Its output is a pure function of immutable public bytes, so an adversary's only
freedom is *whether* a seal exists and *when* it is created. A seal that exists
carries the true verdict, and a seal that does not exist costs a hot action
nothing but the current full validation. Neither is an attack. Correspondingly,
the seal writer must refuse to overwrite: the account is created by
`allocate`+`assign` (never `create_account`, which a lamport-dusting griefer can
break), written exactly once, and thereafter refused as already-sealed.

### Immutability and invalidation

The seal never invalidates within a generation because it never depends on a
generation. `descriptor_digest` is a content identity; the artifacts it names are
content identities; a change to any of them is a different descriptor and a
different seal address. There is nothing to revoke, no staleness window, no
epoch, and no writer that could disagree with an earlier writer. A Trading
upgrade does not invalidate a seal so much as stop addressing it.

A hot action that finds no seal **refuses**. Falling back to full live validation
would be sound, but it would make the frame's account list conditional and leave
two live authentication paths for one fact, which
`AGENTS.md` forbids. The seal is a precondition of a hot execution, exactly as
the Registry activation cache already is, and the operator that builds a hot
transaction is responsible for having sealed the closure first — one transaction,
once, per `(descriptor, action, Trading release)`.

### Rent

The seal is prepaid publication cost, in the same class as the finalized record
it validates, and is paid by whoever submits the sealing transaction. It is never
funded from `FundingStateV1` custody, never from Hoard principal, never from
fees, bounty, insurance or reserve. `AGENTS.md` is explicit that Hoard principal
is none of those things, and the seal is not capability funding either: it is not
per-Market, so no Market's funding could own it without one Market paying for
every other Market's executions. The account is rent-exempt at creation for its
exact fixed width; nothing may reduce it below exemption; and because it is
write-once and content-addressed, there is no growth path and no top-up.

Because the seal is not per-Market, capability closure does **not** close it: a
seal outlives every Market that used it and remains correct. Reclamation is
therefore deliberately out of scope for this decision and is recorded as an
omission rather than half-designed here.

### The refusal when the seal and the live facts disagree

There is one refusal and it is total. If any of the following holds, the hot
action refuses with `TradingSbfError::Content` before any write or CPI:

- the seal account is absent, wrong owner, writable, signer, executable, not
  rent-exempt, the wrong width, or not the exact PDA for
  `(descriptor schema, descriptor digest, action, authenticated Trading semantic
  release, Market-selected Registry)`;
- the seal's magic, schema version, artifact profile or reserved bytes are not
  canonical;
- any row's `schema` or `content_digest` differs from the identity the
  authenticated descriptor names for that role;
- any sealed raw-record or staging address differs from the frame account
  actually supplied at that coordinate;
- any record's live `hash(bytes)` differs from its `content_digest`, or its
  width differs from the sealed width;
- a verdict bit the hot path requires is not set.

There is no "prefer the live fact", no repair, and no re-derivation on mismatch.
A disagreement means one of the two sides is not what it claims to be, and the
only safe response to that is to execute nothing.

## Exact consequences

| Surface | Before | After |
| --- | --- | --- |
| Hot fixed account count | 38 | 39 (one read-only seal account) |
| v0 packet, canonical continuation | 1,224 B | 1,225 B (ALT-routed key, one byte) |
| `find_program_address` per hot action | 2 per record, all records | unchanged except the six sealed rows |
| SHA-256 of record bodies | every record, every action | unchanged — every record, every action |
| New Trading instruction | — | `SealDescriptorClosure`, write-once |
| New PDA domain | — | `CAPABILITY_SEAL_PDA_DOMAIN_V1` |
| Activation route | unchanged | unchanged |
| Capability root header and tail | unchanged | unchanged |

The capability root is deliberately untouched. Its 232-byte header is an
immutable projection of the manifest selection and its tail is the family's
mutable state, handed to family code as `&mut`; putting Trading-common evidence
in either would put two owners in one account and would put evidence inside a
writable region.

## Required refusals

The converged implementation must include adversarial coverage for:

- a seal for a different descriptor digest, a different descriptor schema, a
  different action selector, or a different Trading semantic release, each
  presented at the coordinate the canonical seal would occupy;
- a byte-identical seal at a non-canonical address, and a canonical-address
  account owned by the System Program, the Registry, or Core;
- a writable, signer, or executable seal account, and one below rent exemption;
- truncated, extended, wrong-magic, wrong-version, wrong-profile, and
  nonzero-reserved seal bodies;
- a seal whose row for one role carries another role's schema or digest, and one
  whose rows are correct but permuted;
- a seal whose sealed raw-record address is canonical for its `(schema, digest)`
  but whose account at that coordinate is a different record with a colliding
  width;
- a record whose bytes were substituted after sealing, which must refuse on the
  live body hash and must refuse *before* any sealed verdict is consumed;
- a second `SealDescriptorClosure` against an existing seal, including one
  carrying identical bytes;
- a `SealDescriptorClosure` whose descriptor is finalized but whose named
  artifacts refuse validation, which must leave no account created and no
  lamports moved;
- a lamport-dusted vacant seal PDA, which must still seal;
- a hot action whose seal sets a verdict bit for a join the artifacts do not
  actually satisfy, constructed by sealing one pair and presenting another,
  which the row identity checks must refuse;
- the transfer property stated directly, as
  `ValidatedProfileJoinV3`'s test already states it: a verdict proved for one
  artifact pair does not cover a byte-identical pair at another address, does
  not cover a pair the validator genuinely refuses, and does not cover a second
  policy carrying identical bytes; and
- any refusal after an earlier write or CPI, with transaction-wide rollback
  checked byte-for-byte.

## What this bought, measured

Converged at `ca5e5f1`. Canonical Direct Profile14 bundle, COMPUTE_LIMIT
1,400,000, the real 32,768-byte heap, at the same DP2-owned `Custom(3)` refusal (`TradingSbfError::Content`, `0x4003` since decision 0007)
in the lifecycle preplan, with the suite result unchanged (the same three Direct
emitter failures before and after):

| checkpoint | before | after |
| --- | ---: | ---: |
| artifacts phase | 645,836 CU | 56,693 CU |
| artifacts phase heap | 2,850 B | 2,840 B |
| cumulative to `runtime-observations` | 850,425 CU | 253,391 CU |
| cumulative to the mid-preplan refusal | 1,220,769 CU | 568,486 CU |

**652,283 CU removed at the same refusal point**, against the ~650,000 estimated
below. The heap moved by 10 bytes, as predicted.

Projected against the W2c full-path table, replacing the artifacts phase
(−589,143) and `require_static_register_ownership_v5` (−66,479) and leaving
every per-execution phase unchanged, the path to the child-route preflight
becomes about **1,286,500 CU** of the **1,305,130** a Trading invocation
actually receives under the 1,400,000 protocol limit. That leaves roughly
**18,600 CU** before the three child CPIs, the commit and the acknowledgment
run, and the accumulated heap is still about **61,900 bytes against 32,768**.
The gate is not met and this decision does not meet it. Phases 8 through 10
remain unmeasured because the Direct emitter defect refuses before them.

Writing the seal costs **133,008 CU**, once, per `(descriptor, action, Trading
release, Registry)`.

Estimated saving, itemised:

| removed | CU |
| --- | ---: |
| `AccountProfileV2::decode` sweep | ~101,000 |
| request-profile decode sweep | ~101,000 |
| `EffectProgramV4::decode` sweep | ~259,000 |
| `TransitionProgramV3::decode` sweep | ~20,000 |
| `StateLifecyclePolicyV5::decode_selected` sweep | ~3,000 |
| `CapabilityProgramV4` decode | ~5,500 |
| `validate_account_profile_join` | 82,337 |
| `require_static_register_ownership_v5` | 66,479 |
| six records' `find_program_address` pairs | ~20,000 |
| seal authentication (added back) | ~-8,000 |
| **net** | **~650,000** |

Against the W2c full-path total of 1,942,121 CU to the child-route preflight and
1,304,538 CU actually available to Trading under the 1,400,000 protocol limit,
this leaves roughly 1,292,000 CU — about 12,000 CU of headroom before the three
child CPIs, the commit and the acknowledgment have run. **This decision does not
make the W2 gate pass**, and the heap wall it does not touch at all is 61,889
bytes against 32,768. It removes the single largest structurally removable item
and makes the remaining gap legible: what is left is per-execution projection
work and an allocator that never frees.

## Rejected alternatives

**The chartered shape: extend the activation act to persist the transcript in the
capability-root tail or one sibling PDA.** Rejected on three independent
grounds. First, ground truth: activation authenticates the V1 descriptor
generation and the hot path executes the V4 generation, so activation evaluates
none of the conjunction it would be attesting; an activation-written transcript
today would be an assertion, not evidence, which is precisely the thing this
decision must not become. Second, arithmetic: a program set admits 32 entries,
each with its own closure at ~605,000 CU, so an activation transaction that seals
a Market's actions cannot exist past the second one — and activation already
carries 139 accounts and five CPIs. Third, and decisive even if the first two
were repaired: the cached predicate has no Market in it. Storing it per Market
stores one fact M times, which is the duplication `AGENTS.md` forbids, multiplies
the rent for one fact by the number of Markets, and invents an invalidation
question ("does this Market's transcript still hold?") that the content-addressed
form does not have. Putting it in the root *tail* additionally hands
Trading-common evidence to family code through the `&mut` state slice that
`split_root_account_mut_v1` exists to keep family code away from.

**Persist the decoded artifacts in a Trading-owned account in a cheaper
layout.** The cost is not the layout; it is the validation sweep. A re-laid-out
artifact must still be validated, and it introduces a second byte-level owner for
content whose identity is its bytes.

**Drop the live body hash and rely on the content-addressed PDA plus the
authorized immutable-Registry fast path
(`crates/dclutch-registry-contract/src/immutable_registry.rs`).** This is a
genuinely available and already-authorized argument, and it is rejected here on
value, not on soundness. It saves the body hashes only — about 8,000 CU across
thirteen records — and costs the whole immutable-Registry apparatus: the Registry
artifact-release record, its staging cursor, and the Registry ProgramData
observation must all enter a frame that is one byte from the 1,232-byte packet
limit, and the trust boundary moves from "these bytes hash to this identity" to
"this deployment can never be upgraded". Keeping the hash keeps the strongest
available binding at a price the measurement says is negligible. That fast path
remains unadopted and remains correct; its natural consumer is a reader that must
authenticate a record it will not otherwise read.

**Make the seal a fallback rather than a precondition.** A hot action that
silently re-validates when no seal is present keeps two live authentication paths
for one fact, makes the account frame conditional on chain state the transaction
builder cannot see, and hides the compute cliff between two executions of the
same action. Fail-closed, with the operator responsible for sealing, is the same
discipline the Registry activation cache already imposes.

**Key the seal on the artifact rather than the descriptor closure.** One seal per
`(schema, digest)` is the most granular content-addressed form and is what the
joins forbid: `validate_account_profile_join` and
`require_static_register_ownership_v5` are facts about tuples, and a per-artifact
seal cannot carry them. Keying on the descriptor, which names the whole tuple by
content, carries them exactly. The cost is that eight per-artifact seals would
also have cost eight accounts and eight PDA derivations in a frame that has room
for one.

**Bind the seal to a hand-maintained "sealed validator profile" identity instead
of the Trading semantic release.** Cheaper to upgrade — a Trading release that
changes nothing a validator does would keep its seals — but it makes soundness
depend on a human remembering to bump a constant. The release binding is
fail-closed. Narrowing it later, to an identity that is *emitted* from the
validators rather than asserted beside them, is a real improvement and is
recorded as the lifting plan for this provisional coupling; it is not a reason to
start unsound.

## Convergence file plan

1. `crates/dclutch-capability-seal-contract` — new: `SealedDescriptorClosureV1`
   layout, hostile decode, `CAPABILITY_SEAL_PDA_DOMAIN_V1`, seed projection, the
   invocation-scoped `SealedArtifactV1` token, and its adversarial corpus. The
   byte layout is hand-authored in this crate and must migrate to a Lean-owned
   ABI emitter beside `EmitCapabilityProgramAbiRust.lean`; that migration is the
   lifting plan for its provisional status and is recorded in the omission index.
2. `crates/dclutch-effect-kernel` (`v3.rs`, `v4.rs`),
   `crates/dclutch-account-profile-contract` (`lib.rs`, `lifecycle_v3.rs`),
   `crates/dclutch-request-profile-contract`, `crates/dclutch-transition-vm`,
   `crates/dclutch-capability-program-contract` — each gains exactly one
   `from_sealed` constructor beside its `decode`, taking the token, performing
   the same cheap header parse over the same bytes, and skipping only the sweep
   that the token names. No `decode` is weakened and no validation is deleted.
3. `programs/dclutch-trading-sbf/src/hot_v3.rs` — the `DCLTSEL1` handler beside
   the hot path it is the prologue of, so that "the seal writer and the hot
   reader call the same functions" is visible rather than asserted; and the hot
   consumption itself, threading the tokens and keeping every existing refusal.
   The handler was planned as its own module and is not, because every validator
   and every frame accessor it needs is private to `hot_v3`, and widening all of
   them to `pub(crate)` to move 250 lines would have been a larger change than
   the one it avoided.
4. `programs/dclutch-trading-sbf/src/lib.rs` — one new outer tag in the
   entrypoint dispatch.
6. `programs/dclutch-trading-sbf/program-test` — the seal transaction in the
   canonical Direct campaign, and the refusal corpus above.
7. Delete nothing. `outer.rs::process_activation`, `authenticate_vacant_root`,
   `CapabilityRootSeedsV1` and `CapabilityRootHeaderV1` are unchanged.
