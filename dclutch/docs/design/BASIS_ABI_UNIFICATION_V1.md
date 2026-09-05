# Basis ABI unification, v1 — one wire format, one author, and the road to curvature

**Head current at `330bbfaba` (2026-09-04), tree root `/Users/ember/dev/dclutch`.** The body below `## History` is the FRONTIER-2 design of 2026-08-30 with its 2026-08-31 correction, verbatim; this head states what was ruled and what survives.

## What is true now

- **Ruled.** Decision 0029 item 2 (CONFIRMED by ember 2026-09-04): curvature
  stays in scope and the basis kernel is not retired — option C is refused,
  option D (transfer the assurance, not the code) is the recommendation that
  stands. `docs/OMISSION_INDEX.md` O-013 carries the one-line consequence.
- **The two evaluators are still two.** The live `ProductBasisV3` (handwritten,
  load-bearing on chain in two programs) has no Lean ABI owner and no emitted
  conformance corpus; `dclutch-liability-basis-v2-kernel` has both and no
  caller. Degree-0 and degree-1 shapes are live on the wire under a certified
  categorical projection; degrees 2 and 3 — curvature — are the unreached
  capability, and the price-gate certificate is fully implemented and
  unreachable (no instruction accepts one).
- **The "zero CU" claim was falsified by measurement** (2026-08-31): the
  unconditional `admit_selection_v3` on the shared join and the rewritten
  `ProductBasisV3::decode` with its price-gate probe run on every trade, and the
  shared function runs twice per transaction, so the Direct hot path paid about
  +5,013 CU of the +6,876 the margin gates absorbed at the 2026-08-31 re-pin.
  The cheap recovery — hoist the admission call and digest probe to the
  founding caller — is unchartered.
- The `M-4` correction stands: ember's "5 fixed bands is not good enough" was
  recorded as dropped and is partly delivered under another name.

## History

# Basis ABI unification, v1 — one wire format, one author, and the road to curvature

Status: **design, awaiting a ruling.** Not release evidence, not deployment
evidence, and explicitly not a claim that any of it is scheduled. It targets a
cohort later than the one in flight and says exactly which, and why.

Provenance: FRONTIER-2, 2026-08-30. Every measurement below was taken at HEAD
in `~/dev/dclutch` on that date. Where it contradicts
`docs/evidence/ORPHAN_DESIGNS_TRIAGE_2026_08_30.md` — which chartered it — the
contradiction is called out in §0.3 rather than quietly corrected, because
that document is three hours old and its errors are instructive.

---

## 0. What this document decides

### 0.1 The question

`docs/OMISSION_INDEX.md` `O-013` and `U-013` both stay open on one clause:
the tree contains a proved, byte-guarded implementation of degree-1-through-3
B-spline liability bases with an integer no-arbitrage price gate, and nothing
calls it. The obvious instruction — *wire it up* — is wrong, and this document
exists because the reason it is wrong is not obvious.

**The decision requested is: which evaluator is the authority for the
protocol's basis wire, and what does the other one become?**

### 0.2 The answer this document recommends

Neither of the two options the charter offered. In one paragraph:

> The two evaluators are not two implementations of one specification. They are
> **one implementation with no specification** (the live `ProductBasisV3`,
> handwritten, byte-guarded by nothing, load-bearing on chain in two deployed
> programs) and **one implementation with a specification and a conformance
> corpus but no callers** (the kernel, also handwritten, checked against
> Lean-emitted cases). The asymmetry that matters is not which is authoritative
> — it is that one of them has a proof obligation attached and the other does
> not. So: **give the live evaluator what the kernel already has** — a Lean ABI
> owner and an emitted conformance corpus, byte-guarded — and only then extend
> it to degrees 2–3 by porting the kernel's algorithm under that corpus. The
> kernel crate is retained as a **differential reference**, which `O-005`
> explicitly permits, rather than as a second live writer, which it forbids.

The consequence that makes this worth ruling on now: **five of the ten commits
in §9 change no bytes on any wire**, so they are landable concurrently with a
live founding lane, and one of them — pinning the *second, currently unguarded*
author of the basis-kind byte — is both free and a precondition for doing the
rest safely. The charter assumed the whole item was gated behind the cohort.
Half of it is not.

### 0.3 Four premises in the triage doc that measurement contradicts

Recorded first because a reader who starts from the triage doc will otherwise
size this wrongly.

| Triage claim | Measured |
|---|---|
| `:130` the kernel's evaluator is *"Lean-emitted, byte-guarded"* | **False.** `spline.rs` (628 lines) is **handwritten** and says so at `:3-6`. Only `generated_spline.rs` (329) is emitted, and it is **constants and test corpora, no algorithm**. 3,157 of the kernel's 4,491 lines are handwritten — 70% |
| `:137-139` `DCLTPGT1` occurs once, *"not even the Rust"* | **False on the Rust.** The magic is live at `generated_price_gate.rs:29` as a hex byte array, behind ~1,500 lines of Rust. The claim came from an ASCII `rg`, which cannot see a hex-encoded magic. *"No instruction, no account"* is correct and is the real finding |
| `:146-152` blast radius is *"112 `BasisKindV3::` match sites across 61 files"*, with *"six files in `programs/dclutch-claims-sbf`"* | **Misleading by an order of magnitude.** 118 occurrences across 84 files, but of the 13 `match` expressions over the enum only **10 are exhaustive**; the other 3 have fail-closed wildcard arms. `programs/dclutch-claims-sbf` contains exactly **one** exhaustive match, not six — the other five files mention `ProductBasisV3` only in doc comments. See §1.6 |
| `:129` `runtime_v3.rs` is *"~1,700 lines"* | 1,579 |
| `:92-99` the U-013 field table maps payout scale to *"Core Market `basis_scale` (`generated.rs:784`)"* and basis identity to *"`claim_basis` + `liability_basis` (`generated.rs:308-309`)"* | **Both citations are to the wrong structs.** `:784` is a field of `SeriesOpenObservation` and `:308-309` are fields of `Product` — Lean *kernel* structs, neither of which is in `CoreState`'s 360 persisted bytes. `CoreState` carries **no basis field at all**; it pins `identity.product_record`, a digest of a separate account. The layout claim survives — basis identity reaches the chain through the Product record and the Claims aggregate — but by a different route than U-013 says |

None of these changes the recommendation. Two make the work smaller than
advertised, one makes the assurance argument harder, and the last one relocates
where the basis identity actually lives — which matters in §6, because the
account that has room for a new field is not the one U-013 pointed at.

---

## 1. The problem, exactly

### 1.1 There are three evaluators, not two

| | live | kernel | dormant |
|---|---|---|---|
| type | `ProductBasisV3` | `SplineRequestV2` / `RampRequestV2` / product-claims basis | `ProductPayoffV2` |
| where | `crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs` (1,579 lines) | `crates/dclutch-liability-basis-v2-kernel` (4,491) | same crate, `src/lib.rs:365` |
| record | `DCLTPAY3`, schema 3, 256-byte header | `DCLTLBV2` (three families) + `DCLTLNK2` | `DCLTPAY2`, 576 bytes, 64-byte header |
| authorship | handwritten, **no `@generated` marker anywhere in 1,579 lines** | handwritten, checked against Lean-emitted corpora | handwritten |
| byte guard on the evaluator | **none** | none (the corpora are guarded; the algorithm is not) | none |
| dependents | **24**, including `programs/dclutch-claims-sbf` (`Cargo.toml:29`) and `programs/dclutch-core-sbf` (`:49`), both `crate-type = ["cdylib","lib"]` | **zero.** Two `rg` hits across every `Cargo.toml`: the workspace member list (`Cargo.toml:22`) and its own `[package] name`. Empty `[dependencies]`, no `[dev-dependencies]` | zero outside its own crate; only Lean references it (`LiabilityBasisV2.lean:1` imports `DClutchSemantics.ProductPayoffV2`) |

Read that table's last two rows together, because they are the whole problem:

> **The evaluator that is load-bearing on chain has no specification and no
> byte guard. The evaluator that has a specification, a byte guard and 221
> theorems is linked into nothing.**

That is not a wiring gap. It is an assurance inversion.

### 1.2 "Prove the two evaluators agree" is currently ill-posed

The charter offered *"prove the two evaluators agree and keep both"* as
option (a). It is not available as stated, and the reason is decidable at the
wire rather than a matter of judgement.

**No byte string decodes on both.** `ProductBasisV3::decode`
(`runtime_v3.rs:237`) requires magic `DCLTPAY3` and `len >= 256` with the
header's `record_bytes` equal to `bytes.len()` (`:238-253`).
`decode_spline_request_v2` (`spline.rs:213`) requires magic `DCLTLBV2` and
`len == 144` **exactly** (`:214-219`). Disjoint magics *and* disjoint lengths.
The intersection of their domains is empty.

**Neither implements the other's core notion.** Two greps, run with word
boundaries because a careless case-insensitive search finds `tent` inside
`CONTENT_ID_BYTES_V2`:

- `rg -i 'de.?boor|degree|spline|bspline'` over
  `crates/dclutch-product-payoff-v2-codec/src/` — **zero hits.** The live
  evaluator has no notion of degree at all.
- `rg '\b(Constant|Tent|RampUp|RampDown)\b'` over
  `crates/dclutch-liability-basis-v2-kernel/src/` — **zero hits.** The kernel
  has no shape enum.

**And where a shared notion does exist, the types and the semantics diverge:**

| concept | live (`ProductBasisV3`) | kernel (spline) | divergence |
|---|---|---|---|
| knot | `i128` numerator (`runtime_v3.rs:192`) | `i64` (`spline.rs:259`) | 64-bit narrowing; the live range is unrepresentable in the kernel |
| knot denominator | `u64` (`:189`) | `u32` (`generated_spline.rs:15`) | narrowing |
| coordinate | `i128` / `u64` (`:523`) | `i64` / `u32` (`generated_spline.rs:16-17`) | narrowing on both halves |
| payout scale `Q` | `u64` (`:188`) | `u32` (`generated_spline.rs:14`) | narrowing |
| knot ordering | **strictly** increasing; refuses `knot <= prior` (`:322`) | **non-**decreasing; refuses only `prior > next` (`spline.rs:271`) | the kernel admits repeated knots — interior multiplicity, which is the whole point of a spline — and the live evaluator refuses them |
| amplitude | explicit per-term `u64` | none; weights are structural | no counterpart |
| width | stored `u32`, runtime-unbounded (a test at `:1370` runs width 33 over 301 knots) | derived `knot_count − degree − 1`, capped at `SPLINE_MAX_WIDTH_V2 = 10` | the live wire is strictly more capable here |
| failure payout | stored vector + `evaluate_failure` (`:558`) | **no concept** | no counterpart |
| **rounding** | per-term floor, then the complement claim receives `Q − Σ(primary)` (`:548-553`) | **cumulative-floor telescoping**: each claim gets the difference of two consecutive floors of the running weight sum (`spline.rs:376-380`, `:397-413`) | **different functions**, even on inputs both could express |
| arithmetic | exact signed rational over a hand-rolled 256-bit `SignedU256` (`lib.rs:557-561`), all `checked_*`; the sole rounding boundary `interpolation_floor` (`lib.rs:495`) computes a floor by **binary search**, not division (`lib.rs:522-523`) | `u128`, checked; de Boor accumulates in `[u128; 4]` (`spline.rs:314`); the ramp boundary is plain `u128` division (`lib.rs:409-412`) | different overflow surfaces |

The rounding row is the one that forecloses option (a) even on the fragment
where both are defined. They are not two computations of one function.

### 1.3 The one genuine bridge, and exactly how far it reaches

There is a real, load-bearing connection between the two, and it is not a
theorem — it is a **definition**. `LiabilityBasisV2.lean:820-822`:

```lean
def cappedRampComplementFloorBoundaryV2
    (scale : Nat) (elapsed width : Int) : Nat :=
  DClutch.ProductV2.interpolationFloor scale elapsed width
```

The kernel's sole apportionment boundary **is** the Product V2 interpolation
floor, definitionally, and the module says so in its own docstring
(`:13-15`). `LiabilityBasisV2.lean:1` imports `DClutchSemantics.ProductPayoffV2`:
in Lean these are one lineage. Five theorems stand on it — `_le` (`:824`),
`_interior` (`:832`, which unfolds both definitions at `:844`),
`_never_rounds_up` (`:851`), `_residue_lt_one_atom` (`:873`) and `_monotone`
(`:895`).

**That bridge spans exactly one fragment: the two-claim capped ramp at degree
1.** Everywhere else — degree ≥ 2, width > 2, `Tent`, `Constant`, failure
payouts, width > 10 — an agreement statement is not merely unproven. It has no
domain to be stated over.

So the honest form of option (a) is: *prove agreement on the degree-1 two-claim
fragment, and scope everything else as unreachable.* That is worth doing and it
is not a unification.

### 1.4 The retired ABI, and what the retirement actually left

`programs/dclutch-claims-sbf/src/liability_basis_v2.rs` is not a stub and not a
decoder. It is a 152-line re-export surface, a refusal enum and three encoders;
it owns no route and dispatches nothing (`:1-8`). Its obituary block begins at
line **10** (the triage doc cites 11) and is worth reading in full because it is
the clearest statement in the tree of what happened:

> `DCLLBX02` was the last Claims path expecting a Core-owned
> `LinkedBasisRecordV2` (`DCLTLNK2`). The four live basis consumers —
> `founding_v5`, `affine_batch_v2`, `signed_delta_v3`, `protocol_position_v2` —
> converged onto `authenticate_product_basis_v3`, the Registry-owned
> `ProductBasisV3` record Core authenticates when it commits a founding permit.
> This route never did, and it was dead on BOTH ends […]
>
> Its own deletion note queued it behind "whoever retires the V2
> liability-basis kernel". That kernel has an active lane and is not being
> retired, so the queue was an event that would never arrive. Deleted on its
> own merits instead.

**Verified today, both directions:**

- Producers of `DCLLBX02`: **none.** Every hit is documentation, git reflog, or
  this file's own obituary. `crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs:24`
  independently records *"There are no legacy consumers left."* Burial commit
  `086682f`.
- On-chain finalizers of `DCLTLNK2`: **none.** The only surviving code
  reference in the entire tree is
  `crates/dclutch-liability-basis-v2-kernel/src/product_claims.rs:32` — a
  retired ABI spoken by exactly one crate that nothing calls.

The retirement was correct and it was done well. What it left behind is the
present shape: a 1,173-line `product_claims.rs` modelling a record family the
tree deleted around it, inside a crate whose *other* 3,300 lines contain the
only degree-2–3 implementation the project has.

### 1.5 A latent hazard found on the way, which is not the ruling but should not wait for it

**`DCLTLBV2` is the magic of three distinct record families inside the one
kernel crate.**

| family | magic const | bytes | schema | profile |
|---|---|---:|---:|---:|
| Ramp request | `RAMP_MAGIC_V2` (`generated.rs:17`) | 64 | 2 | **1** |
| Spline request | `SPLINE_MAGIC_V2` (`generated_spline.rs:25`) | 144 | 2 | **2** |
| Product-claims basis | `BASIS_MAGIC_V2` (`product_claims.rs:30`) | 128 or 152 | 2 | **1** |

The spline record is disambiguated **correctly** — profile 2 against the ramp's
profile 1 — and `LiabilityBasisV2SplineAbi.lean:1-12` names that as a deliberate
choice. That is the right pattern.

The problem is **ramp versus product-claims basis: they share the full
discriminating triple `(DCLTLBV2, schema 2, profile 1)`.** Nothing in either
header distinguishes them. They are separated *only* by total record length, and
only because both decoders happen to check length before anything else
(`lib.rs:324-326`, `product_claims.rs:1024-1026`).

Today that is safe. The failure mode if it ever stops being safe — a length
guard reordered after the magic check, relaxed to a minimum, or a future record
in this family landing at 64 or 128 bytes — is the worst shape a wire bug takes,
because the two records assign **different meanings to the same offsets** and
every misread field is a plausible small integer:

- offset 12: ramp reads `u32 scale`; product-claims reads `u8 kind`. A scale of
  1 or 2 decodes as a **valid** kind tag (1 = `CategoricalQ1`, 2 = capped ramp).
- offset 16: ramp reads `u32 knot_denominator`; product-claims reads
  `u32 claim_count`. Both small positive integers; no range check catches the
  swap.
- offset 20/24: ramp reads `i64 left_numerator`; product-claims reads
  `u32 body_bytes` and `u64 scale`. A signed coordinate reinterpreted as a
  length.

**Not a refusal — a successful decode as the wrong family with plausible
values.** The mitigation is one byte: give the product-claims basis its own
profile tag or its own magic. The spline record already demonstrates the fix.

This matters to the ruling in one specific way: **the live evaluator's magic
namespace is clean.** `DCLTPAY3` and `DCLTPAY2` are distinct from each other and
from everything above. A unification that keeps the live magic scheme inherits
no collision; one that adopts the kernel's inherits this one.

### 1.6 What is *not* the problem: the blast radius is 9, not 116

The triage doc's *"112 `BasisKindV3::` match sites across 61 files"* reads as
weeks of mechanical work. Measured today the raw counts are larger — **116 sites
across 84 files** (`crates` 85, `programs` 15, `tools` 12, `docs` 4) — and
almost entirely irrelevant, because the overwhelming majority are *construction*
sites (`BasisKindV3::CategoricalQ1` used as a value), which a third variant does
not touch.

The number that matters is how many `match` expressions over the enum a third
variant would break. Measured with `ast-grep` over `match $S { $$$ARMS }`,
filtered to arms mentioning the enum, then checked for a `_` arm: **13 match
sites, 10 exhaustive, 3 with wildcards.**

**Ten exhaustive matches (a third variant breaks the build — the good case):**

| site | what it does |
|---|---|
| **`programs/dclutch-claims-sbf/src/terminal_certificate_v3.rs:86`** | **the only on-chain one** — `match (basis_kind, certificate.kind)`, §1.7 |
| `crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs:114` | `tag()` → the wire byte |
| `…/runtime_v3.rs:289` | `validate()`, per-kind canonical form |
| `…/runtime_v3.rs:621` | failure-vector width |
| `…/runtime_v3.rs:661` | rounding-boundary byte, encode path |
| `…/runtime_v3.rs:746` | `validate_input`, per-kind input rules |
| `crates/dclutch-rational-representation-v2-kernel/src/product_v3.rs:229` | `to_bytes()` → the `DCRPADV3` kind byte |
| `crates/dclutch-rational-representation-v2-operator/src/lib.rs:1445` | the paired match again, off-chain |
| `tools/local-validator/bootstrap/successor/src/wallet_terminal.rs:1033` | driver |
| `crates/dclutch-product-runtime-v2-svm-reader/tests/reader.rs:221` | test |

**Three wildcard matches — and every one is fail-closed, which is the decisive
sizing fact in this document:**

| site | wildcard arm | what a third kind does |
|---|---|---|
| `crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs:121` | `_ => Err(Error::UnsupportedKind)` | `decode(tag: u8)` — kind 3 is simply **undecodable** until an arm is added |
| `crates/dclutch-rational-representation-v2-kernel/src/product_v3.rs:179` | `_ => Err(Error::NonCanonical)` | the `DCRPADV3` admission byte refuses |
| `crates/dclutch-claims-svm/src/product_basis_terminal_v3.rs:582` | `_ => Err(Error::ProductBasis)` | **the Claims settlement evaluator** — the record decodes, then dies at evaluation with a generic refusal |

> **A third `BasisKindV3` variant cannot silently mis-evaluate anywhere in this
> tree.** Every one of the thirteen sites either fails to compile or refuses at
> runtime. There is no site where a new kind takes a permissive default.

That property turns the enum half of this work from *weeks and frightening*
into *days and boring*. The weeks are in the evaluator, not the enum — exactly
backwards from how the triage doc sized it. The one caveat worth carrying into
the hostile table: the third wildcard is *late* fail-closed. A kind-3 record
would decode successfully and then refuse with a generic `ProductBasis` error at
settlement, which is a confusing failure to debug and is why hostile 19 exists.

### 1.6.1 The kind tag has four independent authors, and one of them is guarded by nothing

This is the part that makes §5's recommendation urgent rather than tidy.

| author | what it asserts | guarded? |
|---|---|---|
| `crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs:60-61` | tags 1 and 2, **handwritten** | **no** — no `@generated` header, invisible to the emission census, no byte guard |
| `crates/dclutch-rational-representation-v2-kernel/src/generated_product_v3.rs:6-7` | `PRODUCT_REPRESENTATION_CATEGORICAL_KIND_V3 = 1`, `…GRADED_KIND_V3 = 2` — **Lean-emitted** | **no.** An exhaustive search for anything re-running `EmitProductRepresentationV3AbiRust.lean` finds one hit: the lake build declaration. It can be hand-edited or drift behind its Lean source silently |
| `apps/dclutch-web/lib/{rationalTerminalHotV3,directHotChain}.ts` | `if (tag !== 1 && tag !== 2) throw`, and a hardcoded *shape* whitelist at `directHotChain.ts:279-281` | no |
| `packages/dclutch-sdk/lib/…` | **byte-identical copies** of the above (`diff -q` reports IDENTICAL for four files) | only `tools/release/final-generated-convergence.py:25-27` notices apps/packages divergence, and only under `lib/generated/` — not these handwritten decoders |

Eleven TypeScript files under `apps/` and `packages/` decode `ProductBasisV3`,
and both reserved ranges are mirrored there too
(`rationalTerminalHotV3.ts:156`, `directHotChain.ts:217-218`:
`requireZero(bytes, 18, 2)` and `requireZero(bytes, 208, 48)`).

**Four authors of one wire tag, of whom the on-chain one has no specification
and the specified one has no guard.** This is the same defect class as §1.1's
assurance inversion, one level down.

### 1.6.2 The schema identity is derived from a name, not from the layout

`GRADED_BASIS_RECORD_SCHEMA_ID_V3` (`generated_admission_v3.rs:18-21`) is
`sha256("dclutch/schema/product-runtime-graded-basis-v3")`. It is a hash of the
**name string**, not of the layout bytes.

> **Consequence, and it is a hard constraint on §6: adding a third kind without
> bumping that name reuses one schema identity for two different body languages,
> and nothing in the tree notices.** A Registry record finalized under the old
> schema id would be accepted by a new decoder that reads the reserved bytes as
> degree.

So the kind addition is *not* purely additive at the identity layer even though
it is additive at the byte layer. §6.1 carries the consequence.

### 1.6.3 The ELF-digest radius is eight of ten programs

Only one `programs/*/src` file holds an exhaustive match, but the release
radius is the link graph, not the match graph. Reverse-dependency closure over
every `Cargo.toml` onto the three crates holding exhaustive matches, intersected
with the release roster (`tools/release/plan-sbf-release-batch.py:26-43`, ten
roles): **eight of ten releasable programs take a new ELF digest** — core,
claims, trading, resolution, custody, dealer-accelerator, general-accelerator,
series-shadow. Only registry and rent are untouched.

That is a cohort fact, not a code fact, and §10 spends it.

### 1.7 The paired match, and its off-chain twin

`programs/dclutch-claims-sbf/src/terminal_certificate_v3.rs:86-105` matches the
pair `(basis_kind, certificate.kind)` with no `_` arm.
`ResolutionCertificateKindV2` has exactly four variants
(`crates/dclutch-resolution-codec/src/v2.rs:98-107`) and `TerminalScenarioV3`
exactly three (`product_v3.rs:383-395`). A third kind needs:

| arm | must answer |
|---|---|
| `(K3, ResolutionSuccess)` | which `TerminalScenarioV3`? **If none of the three fits, `TerminalScenarioV3` gains a variant** — which cascades into `product_basis_terminal_v3.rs:582` (wildcard, so it silently refuses) and `wallet_terminal.rs:1033` |
| `(K3, ResolutionFailure)` | `Failure`, or a K3-specific failure payout |
| widen `:102` to include `K3` | `RecoveryAdvanced` / `Exhausted` → `Err(Identity)`; mechanical |

**This match is duplicated verbatim off-chain** at
`crates/dclutch-rational-representation-v2-operator/src/lib.rs:1445-1467` — same
four arms, same structure, `Error::InvalidTerminal` where the program says
`ClaimsSbfError::Identity`. **Both must move in one commit or the operator and
the program disagree about what the chain accepts** — a second author of the
settlement rule, and the one most likely to be forgotten.

---

## 2. Ground truth the design rests on

### 2.1 The live record, `DCLTPAY3`

256-byte header (offset constants at `runtime_v3.rs:23-58`):

| off | size | field |
|---:|---:|---|
| 0 | 8 | magic `DCLTPAY3` |
| 8 | 2 | `u16` schema = 3 |
| 10 | 2 | `u16` header_bytes = 256 |
| 12 | 4 | `u32` record_bytes, must equal `bytes.len()` |
| 16 | 1 | `u8` kind — 1 `CategoricalQ1`, 2 `GradedExactComplement` |
| 17 | 1 | `u8` rounding-boundary tag (0 or 1) |
| **18** | **2** | **reserved, zero-enforced** |
| 20 | 4 | `u32` basis_width |
| 24 | 4 | `u32` knot_count |
| 28 | 4 | `u32` term_count |
| 32 | 32 | `product_id`, nonzero |
| 64 | 32 | `result_domain_id`, nonzero |
| 96 | 32 | `coordinate_domain_id`, nonzero |
| 128 | 32 | `result_unit_id`, nonzero |
| 160 | 8 | `u64` payout_scale `Q` |
| 168 | 8 | `u64` knot_denominator |
| 176 | 32 | `evaluator_release_id`, nonzero |
| **208** | **48** | **reserved, zero-enforced** |

Tail (`:591-611`, `:615-630`): `failure_payouts` (`basis_width × u64`, graded
only) at 256, then `knots` (`knot_count × i128` LE, 16 B each), then `terms`
(`term_count × 32 B`).

Term layout (`encode_term :1048`, `decode_term :1083`): `+0 u32 claim_index`;
`+4 u8` shape tag (0 `Constant`, 1 `RampUp`, 2 `RampDown`, 3 `Tent`);
`+5..8` reserved-zero; `+8 u32 left`; `+12 u32 peak`; `+16 u32 right`;
`+20..24` reserved-zero; `+24 u64 amplitude`.

The reserved regions are genuinely enforced — `runtime_v3.rs:254-255`:

```rust
require_zero(bytes, HEADER_RESERVED_OFFSET, 2)?;
require_zero(bytes, HEADER_TAIL_RESERVED_OFFSET, 48)?;
```

`require_zero` is at `:1229-1240` and returns `Error::NonCanonicalReserved`; a
test at `:1463-1468` flips byte 208 and asserts it. **The triage doc's "50
reserved bytes at offsets 18 and 208" verifies exactly.**

### 2.2 The price-gate certificate: it has Rust, it has no wire

`DCLTPGT1` is 320 bytes. Lean (`LiabilityBasisV2PriceGateAbi.lean:60-89`) and
Rust (`generated_price_gate.rs:3-29`) agree field for field:

| off | size | field |
|---:|---:|---|
| 0 | 8 | magic `DCLTPGT1` |
| 8 | 2 | `u16` schema = 1 |
| 10 | 2 | `u16` profile = 1 |
| 12 | 4 | `u32` scale |
| 16 | 8 | `u64` mass |
| 24 | 1 | `u8` degree |
| 25 | 1 | `u8` width |
| 26 | 1 | `u8` atom_count |
| 27 | 13 | reserved, zero |
| 40 | 80 | `prices` — 10 × `u64` |
| 120 | 80 | `weights` — 10 × `u64` |
| 200 | 80 | `numerators` — 10 × `i64` |
| 280 | 40 | `denominators` — 10 × `u32` |

Capacity 10 is affine Carathéodory, not an arbitrary cap
(`LiabilityBasisV2PriceGateAbi.lean:17-20`). `PRICE_GATE_EXEMPT_DEGREE_V1 = 1`
(`generated_price_gate.rs:27`, re-exported `price_gate.rs:86`, mirrored in Lean
at `LiabilityBasisV2PriceGate.lean:721`) and is enforced at
`price_gate.rs:396-398`: degree above the exempt degree without a certificate
refuses; a certificate that *is* offered is verified regardless of degree.

The hull check itself (`price_gate.rs:339-371`) recomputes every atom **through
the production evaluator**, never off the wire, and checks
`price_i × mass == Σ(weight × payout_i)` componentwise in `u128`. No division,
no rounding.

**So the certificate is fully implemented and completely unreachable.** There is
no instruction that accepts one, no account that holds one, and no route that
requires one. That is the true statement the triage doc was reaching for.

### 2.3 What ships today — the `M-4` correction, restated because the ruling depends on it

`docs/evidence/ASPIRATION_LEDGER_2026_08_27.md` `M-4` recorded ember's *"'5 fixed bands' is really not
good enough"* as a **dropped** requirement. It is not dropped. `BasisKindV3`
(`runtime_v3.rs:105`) admits `GradedExactComplement` alongside `CategoricalQ1`,
and `BasisShapeV3` (`:131`) carries `Constant`, `RampUp`, `RampDown` and `Tent`
over runtime-width knots. **Degree-0 and degree-1 shaped payoffs are live on the
wire**, under a certified categorical projection carrying a componentwise
integer error bound (`crates/dclutch-product-compiler/src/noncategorical_v3.rs`).

`M-4` and `O-013` were both amended for this on 2026-08-30 (`1b49b0b9`).

The unreached capability is **curvature — degrees 2 and 3.** That is the thing
this document is about, and it is a much smaller and much more honest claim than
"the B-spline requirement regressed."

---

## 3. Ruled constraints

These are not negotiable by this design; they come from accepted decisions and
they eliminate most of the option space before it is explored.

1. **`O-005`, narrowly stated: one live writer and one persisted truth per
   fact** — *and* it explicitly permits *"read decoders, migrations,
   **differential references**, and non-authoritative measurement ELFs."* This
   is the constraint that makes the recommendation legal and option (a)
   illegal: two evaluators that both decide admission are two live writers; one
   evaluator plus a reference implementation is not.
2. **`O-013` admits certified nonnegative integer partition-of-unity bases and
   nothing weaker.** Any new kind must carry exact partition sum at every width.
3. **Degree ≥ 2 without a valid price certificate is an executable arbitrage.**
   `docs/research/EXPANSION_FRONTIER_2026_08_25.md` §"Slice two": at degree ≥ 2
   the simplex condition stops being the no-arbitrage condition. Degree ≤ 1 is
   exempt **by proof**, not by assumption.
4. **A wire-format change cannot land under a live founding lane.** §10.
5. **The hot path has no compute margin.** TRADE-2 measured the canonical
   four-outcome continuation trade at 1,294,068 CU with 8,006 CU of headroom,
   after a semantically-identical function extraction cost 2,049 CU and turned a
   passing route into an exact-limit failure. Any design that adds per-trade work
   to the hot path is dead on arrival. §6.3 is where this bites.
6. **Byte-identity gates are regenerated and `cmp`-clean for any Lean-adjacent
   change**, and every generated file carries the census-conforming provenance
   header (`3e7863fc` repaired the two that did not).

---

## 4. The options, with honest costs

### Option A — prove the two evaluators agree and keep both

**Cost: unbounded, because the statement does not exist.** §1.2 shows the
domains are disjoint at the wire and the rounding rules differ where they
overlap. The reachable version — prove agreement on the degree-1 two-claim
fragment, where `LiabilityBasisV2.lean:820-822` already makes the boundaries
definitionally equal — is worth **days** and is folded into the recommendation
as commit 2. It does not unify anything and it does not unlock curvature.

**And keeping both as live authorities violates `O-005` directly.** Two
evaluators that both decide what a Market may select are two live writers of one
fact. That is the defect class this project names as its own signature, at its
largest scale.

### Option B — retire the handwritten evaluator; make Lean the single author

Superficially the principled answer, and it is **not viable as stated**, for
reasons that are about capability rather than taste:

- The live evaluator has **24 dependents** including two deployed cdylibs. The
  kernel has zero.
- The kernel is **strictly less capable on the live wire's own terms**: width
  capped at 10 against a live evaluator tested at width 33 over 301 knots;
  `i64`/`u32` where the live wire carries `i128`/`u64`; **no failure-payout
  concept at all**; no per-term amplitude.
- Its rounding rule is different (cumulative telescoping versus per-term floor
  plus complement), so adopting it silently changes payouts for every existing
  shaped basis.

Retiring A for B replaces a live, more capable wire with a less capable one and
changes the money. **Cost: a wire migration of every existing Market plus a
capability regression.** Rejected.

There is a *real* option B′ hiding inside it — make Lean the single author of
the live evaluator's ABI and algorithm — and that is the recommendation. The
error in B is the assumption that "Lean as author" requires "the kernel crate as
implementation."

### Option C — retire the kernel path

**Cost: 221 sorry-free theorems across 5,181 lines of Lean, the only
degree-2–3 implementation in the project, and the entire no-arbitrage price
gate.** If curvature is ever wanted, this is the most expensive possible move,
because the proofs are the hard part and they already exist.

It is the right answer *only* if the ruling is that curvature is out of scope
permanently. That is ember's call, not a lane's, and §11 puts it to him
directly.

### Option D — **the recommendation.** Transfer the assurance, not the code

Neither evaluator is retired and neither is proved equal to the other, because
the trichotomy above hides the actual asymmetry: **one has a specification and a
conformance corpus, the other has callers.** The unification that buys something
is to give the evaluator with callers the thing the other one has.

Concretely:

1. **The live `ProductBasisV3` ABI gets a Lean owner and an emitted conformance
   corpus, byte-guarded** — exactly the shape the kernel already has, and
   exactly the shape `EmitCapabilityProgramAbiRust.lean` demonstrates one crate
   over. The evaluator stays where it is and keeps its callers. Its layout stops
   being handwritten. **No wire bytes change.**
2. **The degree-1 two-claim agreement is proved**, using the definitional bridge
   that already exists.
3. **The kernel's de Boor is ported into the live evaluator under that corpus**,
   widening its types to the live wire's (`i128`/`u64`) rather than narrowing the
   wire to the kernel's, and adopting the live rounding rule so no existing
   payout moves.
4. **The kernel crate is retained as a differential reference** — permitted
   explicitly by `O-005`, and genuinely useful: it is an independent handwritten
   implementation of the same mathematics, which is the strongest cheap check
   available on a de Boor port.
5. **Only then** the wire change: a third `BasisKindV3` variant and a
   Registry-finalized slot for the price certificate.

Cost: **weeks**, honestly — but front-loaded with three commits that change no
wire bytes and can land under a live founding lane, which is the property that
makes it schedulable at all.

---

## 5. Recommendation

**Adopt option D.** The one-sentence form for a ruling:

> The live `ProductBasisV3` evaluator is and remains the sole authority for the
> protocol's basis wire. Its ABI moves to a Lean owner with an emitted,
> byte-guarded conformance corpus. Degrees 2–3 arrive by porting the kernel's
> algorithm into it under that corpus, at the live wire's widths and the live
> wire's rounding rule. `dclutch-liability-basis-v2-kernel` is retained as a
> non-authoritative differential reference under `O-005`, and its
> `product_claims.rs` — which models the retired `DCLTLNK2` family — is deleted.

Three things this ruling buys that are worth naming:

- **It resolves the assurance inversion in the direction that reduces risk.**
  Today the code that runs on chain is the code with no specification. After
  commit 1 it is the code with a specification, and that commit ships no wire
  change.
- **It does not spend the 221 theorems.** They become the specification of the
  thing that runs, rather than of the thing that does not.
- **It makes the remaining gap legible.** After commit 3 the only thing standing
  between the tree and curvature is one enum variant and one account, both
  measured in §1.6 and §6.

---

## 6. The wire, specifically

### 6.1 The third `BasisKindV3` variant

```rust
pub enum BasisKindV3 {
    CategoricalQ1,            // header byte 1
    GradedExactComplement,    // header byte 2
    SplineDegree2To3,         // header byte 3   <- new
}
```

It lands in the **existing header**, not a new one:

- the kind byte at offset 16 already carries a `u8` and admits values 1 and 2;
- degree needs one byte and interior-multiplicity permission needs one bit —
  both fit in the **2 zero-enforced reserved bytes at offset 18**, which are
  refused-on-nonzero today (`runtime_v3.rs:254`), so an old decoder confronted
  with a new record **refuses rather than misreads**. That is the property that
  makes this additive change safe, and it is why the reserved bytes were put
  there.
- knot count, knot denominator, `Q`, widths and the knot vector are already in
  the header and tail at wider types than the kernel needs.

**This is the tree's own established pattern, not an invention.** Measured
precedent, three instances:

- **`ProductBasisV3` already is one** — a `u8` kind byte at offset 16 with a
  per-kind derived width, an `_ => Err(UnsupportedKind)` decode arm, a rounding
  byte that must *agree with* the kind (`:291-294`, `:302-306`), and kind-inactive
  fields forced canonical rather than left free (`:296-300`: categorical must
  carry `payout_scale == 1`, `knot_denominator == 1`, `knot_count == 0`,
  `term_count == 0`, else `NonCanonicalReserved`). A third kind must extend all
  four of those, not just the tag.
- **The kernel did exactly this for the spline** — same magic `DCLTLBV2`, same
  schema 2, new `SPLINE_PROFILE_V2 = 2` against the ramp's profile 1, 144 bytes
  against 64, decoders checking magic → schema → profile and refusing with
  `UnsupportedProfile`.
- **The largest deployed instance is `DCLTAP02`** — thirteen profiles under one
  magic with a **profile-dependent header width**
  (`crates/dclutch-account-profile-contract/src/v2.rs:798-818`, `fn header_bytes`
  at `:1219` returning 32/36/40/48/computed). Older profiles keep a reserved-word
  zero check that newer ones spend.

**The rule the tree follows, stated so this design can be checked against it:**
*a new body form gets a new profile or kind byte under the existing magic; a new
record class gets a new magic; nothing in this tree has ever dual-decoded.* The
kind byte here and the separate `DCLTPGT1` magic in §6.2 are both on the right
side of that rule.

**What must relax:** the strictly-increasing knot check
(`validate_knots`, `runtime_v3.rs:323`, `Error::UnorderedKnots`) must admit
repeated interior knots for the new kind only, because interior multiplicity is
how a spline lowers continuity. It stays strict for the two existing kinds — a
per-kind rule, not a global relaxation.

**What must also change, and is easy to miss: the schema identity.** By §1.6.2,
`GRADED_BASIS_RECORD_SCHEMA_ID_V3` is `sha256` of the *name*
`"dclutch/schema/product-runtime-graded-basis-v3"`. A record whose kind byte may
now be 3 is a different body language under the same identity. **The name must
be bumped (`…-v4`) and the schema id re-derived in the same commit as the kind
byte**, or a record finalized under the old identity is accepted by a decoder
that reads offset 18 as a degree. This is the one place where the change is not
additive, and nothing in the tree would catch it.

**What must not change:** the 48 reserved bytes at offset 208 stay reserved and
zero-enforced *for the kind byte's purposes*. §6.2 spends 32 of them, and that
is the last spend this record gets — leaving the wire with no slack is the
`CoreState` situation (`STATE_BYTES = 360`, last field ends at exactly 360, no
reserved bytes at all, no `require_zero` anywhere in the decoder, and 25
`data.len() != STATE_BYTES` refusal sites across `programs/*/src`) that makes a
field addition there an account-size migration.

Reserved bytes have been spent in this tree exactly once before — W2q,
2026-08-27, four bytes of `CapabilityRootHeaderV1`'s reserved word — and it was
allowed because *"no offset moved, no width changed and nothing regenerated"*
and the sole writer was named. §6.2 meets the same three conditions.

### 6.2 The price certificate's wire slot

The certificate is 320 bytes and needs a home. **It cannot be inlined
anywhere** — a field-map walk of every record in this family shows each is
gapless and exactly its declared width, and the largest free run in any of them
is 48 bytes:

| record | magic | bytes | reserved (zero-enforced) | free |
|---|---|---:|---|---:|
| `ProductBasisV3` header | `DCLTPAY3` | 256 | `18..20`, `208..256` | **50** |
| `ProductBasisV3` term | — | 32 | `+5..+8`, `+20..+24` | 7 / term |
| `CoreState` | `DCLTCOR3` | 360 | **none** | **0** |
| `LiabilityBasisMarketV2` header | `DCLLBM02` | 256 | `10..12` | 2 |
| `LiabilityBasisPositionV2` header | `DCLLBP02` | 128 | `10..12`, `120..128` | 10 |
| `ProductRepresentationAdmissionV3` | `DCRPADV3` | 528 | `11..16`, `500..504` | 9 |
| `ApproximationCertificateV3` | `DCLTAPX3` | 256 | `11..16`, `232..256` | 29 |
| `GradedBasisAdmissionV3` | `DCLTGAD3` | 304 | `10..16` | 6 |
| `PriceGateCertificateV1` | `DCLTPGT1` | 320 | `27..40` | 13 |

And instruction data is not an option either: the canonical continuation packet
is already 1,225 B against the 1,232 B limit (decision 0005's own consequences
table). 320 bytes do not exist there.

**So the design is forced, and the forced answer is the good one:**

> **The certificate is its own Registry-finalized record under its own magic
> `DCLTPGT1`, and `ProductBasisV3` carries a 32-byte digest of it in the
> `208..256` reserved tail.**

That is the tree's own rule from §6.1 applied twice — a new record class gets a
new magic; the reference to it is an additive field in reserved space. It costs
32 of the 48 reserved bytes, leaves 16, moves no offset, changes no width, and
regenerates nothing else.

The certificate account is authenticated exactly the way `ProductBasisV3`
already is: Registry-owned, `!is_signer`, `!is_writable`, `!executable`,
rent-exempt for its exact 320-byte width, a vacant System-owned staging cursor,
and `hash(bytes) == content_digest` — where the digest is the one the
**authenticated basis record** carries in its tail, never one the caller
supplies. That last clause is what makes the binding sound: the caller chooses
which account to pass, and the basis record chooses which digest it must have.

### 6.3 Admission is at **founding**, once — never on the hot path

This is the design's most important consequence and it follows from constraint 5.

A Market's basis is fixed at founding. `authenticate_product_basis_v3` is
already the join Core performs when it commits a founding permit
(`liability_basis_v2.rs:13-16`), and `founding_v5` is already one of the four
live basis consumers. So:

> **The degree-≥2 price gate is a founding-time admission conjunct. Core refuses
> to found a Market whose basis declares degree ≥ 2 without a valid certificate.
> No trade ever verifies a certificate, and the hot path gains exactly zero
> CU.**

The alternative — verifying at trade time — would put a hull check with ten
`u128` multiply-accumulates onto a route with 8,006 CU of headroom, and would
re-verify on every trade a fact that cannot change. It is both more expensive
and less sound (a Market could exist in an unadmitted state between founding and
first trade).

`PRICE_GATE_REFUSAL_BUFFER_V1 = 321` (`generated_price_gate.rs:26`) suggests the
kernel already anticipated a 320-byte payload with a one-byte tag; the
implementing lane should confirm that reading rather than assume it.

---

## 7. Hostiles, with codes reserved

Refusal codes follow decision 0007: `band = code >> 12`, each band `0x1000`
wide, discriminants written as literal hex, and two compile-time assertions per
enum (starts at its registered base, last variant below `base + BAND_SPAN`). The
registry (`crates/dclutch-refusal-registry/src/lib.rs`) is authoritative over
the ADR by the ADR's own words.

**Basis refusals do not live in one band, and that is correct rather than
untidy** — a refusal belongs to the program that makes it. Three programs refuse
in this design, so three allocations are needed:

| refusal | program | band | taken | **reserved here** |
|---|---|---|---|---|
| record decode / admission (unsupported kind, bad degree, width derivation) | `dclutch-product-runtime-v2-sbf`, `AdmissionSbfErrorV2` (`src/lib.rs:38-56`) | `0x9000` | `0x9000`–`0x9008` | **`0x9009`, `0x900A`, `0x900B`** |
| founding-time price-gate conjunct (§6.3) | `dclutch-core-sbf` | `0x3000` | `0x3000`–`0x3011` | **`0x3012`–`0x3016`** |
| terminal settlement (the kind-3 arm at `terminal_certificate_v3.rs:86`) | `dclutch-claims-sbf`, `ClaimsSbfError` (`src/lib.rs:166-206`) | `0x5000` | `0x5000`–`0x500B` | **`0x500C`** |

These extend three existing enums rather than opening a new sub-band. A new
round sub-band (`0x5300` is the lowest free one in band 5) is the right shape
for a new *request family* with its own instruction; basis admission is not
that — it is a new refusal reason inside routes that already exist.

Two allocation hazards, both measured:

- **Do not take `0x5600`–`0x560A` or `0x5620`–`0x5625`.** They are soft-reserved
  by `docs/design/CLAIM_CHECK_COMPACTION_V1.md:965,941` for an unlanded design,
  and **nothing enforces that reservation** — the census checks uniqueness among
  codes that exist, not among codes a design doc has spoken for.
- **`0x4008` is claimed in the working tree** (an uncommitted
  `TradingSbfError::HeapFrame` from the wall-#27 heap work).

And one caveat the implementing lane must not skip, because it is work rather
than paperwork: **the payoff codec's error enums carry no discriminants and no
`#[repr]`** (`runtime_v3.rs:65`, `registry_v3.rs:30`, `product_v3.rs:48`). That
is correct under decision 0007 §4 for an enum that only maps into another — but
it means `ProductBasisV3::Error::UnsupportedKind` **has no wire code today**, and
Claims collapses every basis failure into `LiabilityBasisSbfErrorV2::ProductLink
= 0x5104`. A hostile corpus cannot currently tell "reserved byte nonzero" from
"knot order wrong" from "width mismatch" through the ELF. Half the table below is
untestable on chain until those codes exist.

(The kernel's own `tag() -> u8` numbering 0..=31, with price-gate guards at
20–31 and `PriceGateRequired = 31` matching `PRICE_GATE_REQUIRED_TAG_V1`, is an
**unrelated** internal vocabulary and is not a namespaced wire code. Do not
mistake tag 31 for a refusal code.)

| # | Hostile | Must refuse with |
|---:|---|---|
| 1 | A `DCLTPAY3` record with kind byte 3 presented to an **old** evaluator | `NonCanonicalReserved` — the degree byte lands in the offset-18 reserved region, which old code refuses on nonzero. **This is the negative control for the whole migration and it must be run against the pre-change ELF.** Note it must be *written*, not inherited: no test anywhere in the tree currently plants an unknown kind byte in a `ProductBasisV3` record, so there is no forward-compatibility refusal test to build on |
| 2 | Kind 3 with degree 0 or 1 | `0x9009` `BasisDegreeOutOfProfile` |
| 3 | Kind 3 with degree 4 or 255 | `0x9009` |
| 4 | Kind 3, degree ≥ 2, **no certificate account supplied** at founding | `0x3012` `PriceGateRequired` |
| 5 | Kind 3, degree ≥ 2, certificate whose `hash(bytes)` ≠ the digest in the basis record's tail | `0x3013` `PriceGateBasisMismatch` |
| 6 | A certificate whose hull identity fails: `price_i × mass ≠ Σ(weight × payout_i)` for one `i` | `0x3014` `PriceGateHullRefused` |
| 7 | A certificate with `atom_count > 10` | `0x3015` `PriceGateCapacity` |
| 8 | Nonzero bytes in the certificate's 13 reserved bytes at offset 27 | `0x3016` `PriceGateNonCanonical` |
| 9 | A **byte-identical** certificate at a non-canonical address | `0x3013` — the digest is read from the authenticated basis record, never from the caller |
| 10 | A certificate account owned by System, Core, or Claims rather than the Registry | `0x3016` |
| 11 | A writable, signer, or executable certificate account | `0x3016` |
| 12 | A certificate below rent exemption for exactly 320 bytes | `0x3016` |
| 13 | **Repeated interior knots on kind 1 or 2** (the relaxation must not leak) | existing `Error::UnorderedKnots` (`runtime_v3.rs:323`) — **negative control: this must still fail after the relaxation lands** |
| 14 | Kind 3 with `knot_count − degree − 1 ≠ basis_width` | `0x900A` `BasisWidthDerivationMismatch` |
| 15 | Kind 3 whose evaluated partition does not sum to exactly `Q` at some coordinate | `0x900B` `PartitionNotExact` — **conservation, asserted rather than accepted** |
| 16 | Kind 3 with a nonzero certificate digest but degree ≤ 1, or a zero digest with degree ≥ 2 | `0x3012` — the tail field and the degree must agree in both directions |
| 17 | A record finalized under the **old** `…-graded-basis-v3` schema id whose kind byte is 3 | must refuse: §1.6.2. The schema-id bump is what makes this refusable at all, and a test that constructs the old identity with a new body is the proof it happened |
| 18 | A `(kind 3, certificate kind)` pair at `terminal_certificate_v3.rs:86` with no resolution-failure arm | must not compile. Verified by building, not by testing |
| 19 | A kind-3 record reaching `product_basis_terminal_v3.rs:582`'s wildcard | `0x500C` — the **new** settlement code, not the generic `Error::ProductBasis` the wildcard gives today (§1.6). Otherwise the first person to hit it debugs a settlement failure with no signal |
| 20 | The operator's twin at `rational-representation-v2-operator/src/lib.rs:1445` disagreeing with the program about any `(kind, certificate kind)` pair | a differential test over all pairs — §1.7; the two must be moved in one commit |
| 21 | A degree-2 basis founded successfully, then a trade — the trade must verify **no** certificate | asserted by CU measurement: the hot path's cost must be **unchanged** |
| 22 | The differential control: for every case in `SPLINE_AGREEMENT_CASES_V2` (28 cases) expressible on the live wire, the ported evaluator and the retained kernel must agree exactly | a disagreement fails the build |

Hostiles 1, 13, 17, 21 and 22 are the ones that would actually catch a bad
change. 1 and 13 are negative controls that must be run against the *pre-change*
ELF; 17 is the schema-identity trap; 21 is the CU conservation claim; 22 is the
port's only independent check. The rest are shape checks.

---

## 8. Alternatives rejected

**Add a fourth `BasisShapeV3` variant instead of a third `BasisKindV3`.**
Tempting because `BasisShapeV3` already has four variants and shapes feel like
the natural home for "curve". Rejected: shapes are per-*term* and a spline basis
is not a sum of independently-amplitude-scaled terms — its weights are
structural, induced by knots and degree together (§1.2). Encoding curvature as a
shape would require every term to carry the whole knot vector, and would make the
partition-of-unity proof a property of a *collection* of terms rather than of the
record. The kind byte is the right discriminator because degree changes the
*evaluator*, not the *term*.

**Put the price certificate in the capability root's reserved tail.**
Rejected for the same reason decision 0005 rejected putting seal evidence there:
the tail is handed to family code as `&mut`, and
`split_root_account_mut_v1` exists precisely to keep family code away from
common evidence. A no-arbitrage certificate in a writable region is not evidence.

**Make the certificate optional and verify lazily at first trade.**
Rejected on constraint 5 (CU) and on soundness: it admits a window in which a
founded Market carries an unadmitted basis, which is exactly the executable
arbitrage the gate exists to prevent.

**Bump `DCLTPAY3` to `DCLTPAY4` rather than adding a kind.**
Rejected: it forces a dual-decode period across 24 dependent crates, two
cdylibs and eleven TypeScript files for a change the reserved bytes already
accommodate. `O-002` permits a released schema to gain explicit version decoders
— it does not recommend it when an additive discriminator fits.

**Delete `product_claims.rs` in the same commit as the port.**
Rejected on hygiene, not on merit: it is 1,173 lines modelling a retired record
family and it should go, but bundling a deletion with a semantic port makes the
port's diff unreadable. It is commit 6, alone.

---

## 9. Implementation plan, per commit

Every commit compiles; every Lean-adjacent commit regenerates its byte-identity
gate and shows it `cmp`-clean.

| # | Commit | Wire change? | Gate |
|---:|---|---|---|
| 1 | `ProductBasisV3`'s ABI moves to a Lean owner: `DClutchSemantics/ProductBasisV3Abi.lean` + `EmitProductBasisV3AbiRust.lean` + `check-generated-basis-v3.sh`. Every offset constant in `runtime_v3.rs:23-58` becomes emitted. The evaluator body is untouched | **none** | the emitted constants are **byte-identical** to the handwritten ones — a `cmp` against the current file is the proof that nothing moved |
| 2 | The degree-1 two-claim agreement theorem, over the definitional bridge at `LiabilityBasisV2.lean:820-822`. States exactly what it covers and names the rest unreachable | **none** | `lake build`, zero `sorry` |
| 3 | An emitted conformance corpus for the live evaluator, in the shape of `SPLINE_AGREEMENT_CASES_V2`: agreement cases and refusal cases for `CategoricalQ1` and `GradedExactComplement` at several widths | **none** | new byte guard; census count rises with **unguarded unchanged** |
| 4 | A byte guard for `generated_product_v3.rs` — it is Lean-emitted and re-run by nothing (§1.6.1). Second author of the kind tag, pinned | **none** | new `check-generated.sh`; census guarded count rises, **unguarded unchanged** |
| 5 | The `SplineDegree2To3` kind: enum variant, header byte 3, degree + multiplicity into the offset-18 reserved bytes, the per-kind knot-ordering relaxation, **the `…-graded-basis-v4` schema-id bump (§1.6.2)**, all ten exhaustive match sites, and the three wildcard arms given real codes | **YES** | hostiles 1, 2, 3, 13, 14, 16, 17, 18, 19; negative controls 1 and 13 run against the **pre-change ELF** |
| 6 | The paired match and **its off-chain twin together** — `terminal_certificate_v3.rs:86` and `rational-representation-v2-operator/src/lib.rs:1445`, one commit, plus the differential test over all `(kind, certificate kind)` pairs | none beyond commit 5 | hostile 20 |
| 7 | The de Boor port under the corpus, at `i128`/`u64` widths and the live rounding rule; the differential harness against the retained kernel | none | hostiles 15, 22; conservation asserted |
| 8 | Delete `product_claims.rs`; give the product-claims basis its own profile tag if any of it survives (§1.5) | none on the live wire | the retired-family deletion, alone, so its diff is readable |
| 9 | The `DCLTPGT1` finalized-record class, the 32-byte digest in the basis tail, and the founding-time admission conjunct in Core | **YES** | hostiles 4–12, 21 |
| 10 | TypeScript: one author for the layout, or the eleven client decoders pinned against the emitted artifact. Both `apps/` and `packages/` copies, which are byte-identical today and must stay so | none | the SDK/web ABI verification suite |

**Commits 1–4 and 8 are landable now.** Commits 5, 6, 7, 9 and 10 are the
cohort work. Note that commit 4 — pinning the *second* author of the kind tag —
is both free and a precondition for commit 5 being safe, and it is the cheapest
item in this document.

---

## 10. Cohort target and the sequencing constraint

**Commits 1–4 and 8: no cohort. They change no wire bytes and no ELF-visible
behaviour, so they can land under a live founding lane.** This is the single
most useful scheduling fact in the document and it was not visible from the
triage doc's framing, which treated the whole item as one indivisible
wire-format change.

**Commits 5, 6, 7, 9, 10: cohort 9 at the earliest.** Not cohort 8, and the
reason is a chain of facts rather than caution.

First, the size of the cut. Only one `programs/*/src` file holds an exhaustive
match, which invites the conclusion that this is a one-program change. It is
not: by reverse-dependency closure onto the three crates holding exhaustive
matches, intersected with the ten-role release roster, **eight of ten releasable
programs take a new ELF digest** — core, claims, trading, resolution, custody,
dealer-accelerator, general-accelerator, series-shadow. Only registry and rent
are untouched. **This is a near-total redeploy**, and it must be priced as one.

Then the sequence:

1. **Cohort 7 is held.** TRADE-2 owns the cut and it is gated on wall #27 — the
   public Direct route OOMs at ~1,055,000 CU because Hot is deliberately off
   `declares_extended_heap_profile_v1`. ORCH ruled on 08-30 that Hot moves onto
   the extended heap profile, with four binding conditions including a sweep of
   every in-tree caller.
2. **Cohort 8 is already spoken for** by the work named-but-uncommitted at that
   ruling: the ShadowAot certificate rebind, the Fractional activation root-tail
   descriptor, ECON's same-tx founding fold, the Pyth fee ceiling, LIVE-2's Q6
   Claims replay closer, and Q1A's release-lineage migration.
3. **A basis wire change re-founds every Market.** The kind byte is inside the
   record Core authenticates when it commits a founding permit. Changing what
   that record may contain, while a lane is founding market19 against it, is the
   exact hazard `O-002` and the cohort discipline exist to prevent.

**What must be true before commit 5 is written:**

- market19 exists and **has traded** — the acceptance condition the project has
  never met, and the thing that distinguishes a wire we are confident in from
  one we hope about;
- wall #27 is closed and the extended-heap sweep is complete, so the CU and heap
  margins in constraint 5 are known rather than assumed;
- Q1A's release-lineage migration has landed, so a wire change does not strand a
  Market the way cohort-7 strands market18 — the lineage hop is what makes a
  basis migration survivable at all;
- commits 1–4 have landed and their byte guards are green, so the wire change is
  made against a specified evaluator with both of its Rust authors pinned,
  rather than against a handwritten one and an unguarded emitted one.

That last condition is the reason to land commits 1–4 *now* rather than waiting:
they are the precondition for doing the risky part safely, and they carry no
risk themselves.

**One thing that will not be true and should be said plainly: none of these
gates run automatically.** There is no `.github` directory in this tree; the
byte guards run only from an opt-in local `pre-push` hook
(`tools/emission-guard/install-hooks.sh:41-43`) that is bypassable with
`SKIP_EMISSION_GUARD=1`, and the hook is not installed in this checkout. A
design whose safety rests on 22 hostiles and four byte guards, none of which a
push executes, is resting on lane discipline. That is cycle-3 item 4's problem
rather than this document's, but a lane that writes commit 5 without knowing it
is one forgotten `sh check-generated.sh` from a silent ABI drift is a lane that
will eventually forget.

---

## 11. What this hands to ember, and what is left to the implementer

### 11.1 The question for ember, stated so it can be answered

`M-4` recorded his *"'5 fixed bands' is really not good enough"* and *"it was
vital to me to be able to do these properly shaped dynamics"* as a dropped
requirement. **It was not dropped, and asking him to re-authorize something that
already partly ships would waste his answer.** The real question:

> Ramps and tents ship on the live wire today — a Market can select
> `GradedExactComplement` with `Constant`, `RampUp`, `RampDown` or `Tent` terms
> at runtime width right now. What does *not* ship is curvature: degrees 2 and
> 3. Those are proved (221 theorems, zero `sorry`), implemented (integer de
> Boor, no floating point), and gated by a proved no-arbitrage price certificate
> — in a crate that nothing calls, because connecting it means ruling that the
> live evaluator owns the wire and absorbs the kernel's algorithm.
>
> **Is curvature worth weeks, and does it go before or after the first real
> trade?**

Two adjacent questions that belong in the same conversation, because all three
are "ember chooses a posture" and none is unblockable by engineering:
`docs/evidence/ORPHAN_DESIGNS_TRIAGE_2026_08_30.md` §3.2 (does v1 ship one-attempt markets
forever?) and §3.3 with `M-26` (the fee rate).

### 11.2 Left to the implementer

- **The exact `Q` and denominator arithmetic of the port.** The live evaluator's
  `interpolation_floor` is a binary search over a 256-bit sign-magnitude type;
  de Boor accumulates in `u128`. Whether the port widens de Boor to `SignedU256`
  or proves the `u128` accumulation cannot overflow at the live wire's widths is
  a real question with a real proof obligation, and this document deliberately
  does not guess at it.
- **Whether `product_claims.rs` dies entirely or its basis family survives with
  its own profile tag** (§1.5). If it survives anywhere, the magic collision must
  be fixed in the same commit.
- **Whether `ProductPayoffV2` (`DCLTPAY2`, the dormant third evaluator) is
  deleted.** It has zero consumers outside its own crate and only Lean references
  it. It is not load-bearing and it is not this document's to spend.
- **The three code allocations above and the codec's binding to the refusal
  registry** (§7). Without it the hostile table is half-untestable through the
  ELF, and that is work, not paperwork.
- **Whether to add a magic-uniqueness gate while in the neighbourhood.** There
  is none today: `dclutch-route-census --check-unique` checks refusal codes, and
  `tools/gauntlet/census/src/enumerate.rs:80-81` merely *asserts* magics are
  unique by construction. Four accidental collisions already exist in the tree
  (`DCLTPRQ2`, `DCLTSA03`, `DCLTSTV3`, and §1.5's latent `DCLTLBV2` ambiguity
  separated only by exact length). This design introduces a new magic; a lane
  introducing a magic is the right lane to make collisions detectable, but it is
  a scope call rather than an obligation.
- **Whether the whole rational-representation family gets guards.** Five of the
  tree's 41 unguarded generated files are in `rational-representation-v2-*`
  crates, one of which (§1.6.1) is a second author of the very tag this design
  changes. Commit 4 pins one of the five; the other four are adjacent and cheap.

---

*Cross-references: `docs/evidence/ORPHAN_DESIGNS_TRIAGE_2026_08_30.md` §2 and
§4.1 (the measurement this document was chartered from);
`docs/OMISSION_INDEX.md` `O-013`, `U-013`, `O-005`, `O-002`;
`docs/evidence/ASPIRATION_LEDGER_2026_08_27.md` `M-4`, `M-9`;
`docs/decisions/0007-namespaced-refusal-codes.md`;
`docs/research/EXPANSION_FRONTIER_2026_08_25.md` §"Slice two";
`docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md` (three gen-1 deficits —
width 10 vs 16, edge policy, rounding symmetry — that this design does not close
and does not claim to).*

---

## Correction (2026-08-31, measured): §"zero CU" falsified

The claim at line ~735 — "the hot path gains exactly zero CU," with
hostile 21 saying it is "asserted by CU measurement" — is half true and
half never-run. TRUE: `verify_price_gate_v1` never executes on a
zero-digest basis. FALSE: the unconditional `admit_selection_v3` on the
shared join (+446 CU) and the rewritten `ProductBasisV3::decode` with
its price-gate probe (+4,567 CU) run on EVERY trade — and the shared
function runs twice per transaction (Trading hot_v3.rs:1999, Claims
sparse_native_transfer_v1.rs:614), so the decode runs 4×. Measured cost
to the Direct hot path: ~+5,013 of the +6,876 the margin gates absorbed
at the 2026-08-31 re-pin (e74b5dd8; floors 1,271,552 / 1,269,919).
The cheap recovery, unchartered: hoist the admission call and digest
probe to the founding caller (~4,500 CU back). The code comment calling
that function "founding's join" is the false premise.
