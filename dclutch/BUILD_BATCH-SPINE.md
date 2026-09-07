# BUILD — family `batch-spine`

Branch `build/batch-spine`, head `a4f8e4c4e`, forked from `main` at `b1c763903`.
Two commits, 6 files, +958/−11. Working tree was clean at the quota wall — the
maker committed everything it wrote and stopped between "the Lean is done" and
"the Rust exists".

Written by CLOSEOUT-C, not by the maker: the maker never reached its deliverable
doc. Everything below is derived from the branch diff and re-checked against the
tree at this HEAD. Where the maker's intent is inferred rather than stated, it
says so.

**The one sentence:** this family is a complete, `sorry`-free Lean design of the
RFQ-as-a-batch-of-two (101 theorems across three modules) whose *only* Rust
consequence is a price-rule change that is already live in the emitted
transition — so the branch is simultaneously **unbuilt** (no V3 intent codec, no
RFQ route) and **already red** (four gates fail at HEAD because the ordinary
program's width moved and its readers did not).

## 1. What exists, file by file

### 1.1 Lean

| file | what |
| --- | --- |
| `formal/dclutch-semantics/DClutchSemantics/DirectRfqV1.lean` (new, 501 lines) | The RFQ as a two-order batch over `JointClearingV1`. 25 defs, **59 theorems** (40 plain + 19 `@[simp]`), 0 `sorry`. `derivedPrice sellerFloor buyerCap := (sellerFloor + buyerCap) / 2` (`:51`) — the equal split, rounded down. `Rfq` (`:119`) carries `outcomeCount, scale, outcome, sellerId, buyerId, sellerFloor, buyerCap, fill`. `Rfq.crossed` (`:209`) is a **nine-conjunct** admission predicate: `2 ≤ outcomeCount`, `outcome < outcomeCount`, `0 < scale`, `buyerCap ≤ scale`, `sellerFloor ≤ buyerCap`, `0 < fill`, `sellerId ≠ 0`, `buyerId ≠ 0`, `sellerId ≠ buyerId`. `complementIndex` (`:137`) and `Rfq.priceVector` (`:143`) place the price on the named outcome and its complement; `Rfq.sellerOrder` (`:151`, delivers `e_outcome`, `limit := -(sellerFloor : Int)`) and `Rfq.buyerOrder` (`:161`, receives `e_outcome`, `limit := buyerCap`) project the two sides; `Rfq.clearing` (`:187`) and `Rfq.batch` (`:205`, `phase := .closed`, `liveOrders := 2`, `clearing := none`); `counterpartyAdmits` (`:217`) binds a ticket's counterparty; `Rfq.ofTickets` (`:226`). The load-bearing theorem is `crossed_clears` (`:335`); `rfq_uniform_price` (`:389`), `rfq_moves_no_sets` (`:379`), `rfq_is_two_full_fills` (`:382`), `net_zero` (`:326`) and `uncrossed_refuses` (`:398`) are the family's laws. |
| `formal/dclutch-semantics/DClutchSemantics/DirectIntentV3Codec.lean` (new, 274 lines) | The V3 compact-intent ticket. 23 defs, **17 theorems**, 0 `sorry`. Magic `DCLTDIW3` (`:33`), version 3, **172 bytes**, 204-byte signed preimage (32 + 172), signature domain `dclutch/signature/direct-compact-intent-v3` (`:38`). Seventeen fields (`:187`); the **only** addition over V2's 140 bytes is `counterparty` at offset 140, width 32 — the field that makes a ticket one half of an RFQ rather than an open offer. `CompactIntentV3.bearer` (`:132`) is an all-zero counterparty; `CompactIntentV3.terms` (`:158`) projects back to a `CompactIntentV2`. `v2_offsets_preserved` (`:199`) and `v3_extends_v2` (`:178`) are the compatibility laws; `legacy_magic_refused` / `v2_magic_refused` / `v2_domain_refused` (`:226`–`:233`) are the hostiles. |
| `formal/dclutch-semantics/DClutchSemantics/DirectOrdinaryV3.lean` (edited, +74/−11, now 899 lines) | 32 defs, **25 theorems**, 0 `sorry`. **The semantic change of the whole branch.** In `preludeOps` (`:541`–`:551`) the two pre-existing conjuncts `sellerLimit ≤ executionPrice ≤ buyerLimit` are kept as a crossing check, and four ops are inserted after them: `loadConst two 2`, `checkedAddInto sellerLimit buyerLimit limitSum`, `mulDivFloor limitSum one two derivedPrice`, `scalarEq executionPrice derivedPrice`. The request's `executionPrice` must now **equal** the equal split, not merely lie in the interval. New scalar slots `limitSum = 68`, `two = 69`, `derivedPrice = 70`. Four new `native_decide` theorems: `a_matcher_price_off_the_equal_split_refuses` (`:733`), `an_agreed_price_admits_unchanged` (`:741`), `the_equal_split_rounds_down` (`:750`, 40/61 clears at 50 and naming 51 refuses), `an_uncrossed_pair_refuses` (`:760`). |
| `formal/dclutch-semantics/EmitDirectIntentV3Rust.lean` (new, 27 lines) | A `def main : IO Unit` printing to **stdout** (like all 94 peer emitters; the target file is fixed by whichever guard redirects it — and no guard does, §2 S4). Prints the provenance header, five public constants, the seventeen `pub(crate)` offsets, and three `#[cfg(test)]` example arrays. Byte-for-byte the committed `generated_intent_v3.rs`. |

No `sorry`, no `axiom`, no `admit` on the branch. (Two `admit` grep hits are the
English word in doc comments.)

### 1.2 Generated Rust

| file | what |
| --- | --- |
| `crates/dclutch-trading/src/generated_intent_v3.rs` (new, 71 lines) | `COMPACT_INTENT_VERSION_V3 = 3`, `COMPACT_INTENT_BYTES_V3 = 172`, `COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V3 = 204`, `COMPACT_INTENT_MAGIC_V3 = DCLTDIW3`, `COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V3: [u8; 32]` (`eb4b19bd…`), then seventeen `pub(crate)` offsets ending at `COMPACT_INTENT_COUNTERPARTY_OFFSET_V3 = 140`, then three `#[cfg(test)]` fixtures (`NAMED`, `NAMED_SIGNED`, `BEARER`). |
| `crates/dclutch-trading/src/generated_ordinary_v3.rs` (edited, +22/−6) | New: `SCALAR_LIMIT_SUM_V3 = 68` (`:139`), `SCALAR_TWO_V3 = 69` (`:141`), `SCALAR_DERIVED_PRICE_V3 = 70` (`:143`). **Moved:** `DIRECT_ORDINARY_COMMON_SCALARS_V3` 68 → **71** (`:215`), `DIRECT_ORDINARY_PRELUDE_INSTRUCTIONS_V3` 69 → **73** (`:223`), `DIRECT_ORDINARY_TRANSITION_INSTRUCTIONS_V3` 73 → **77** (`:229`), `DIRECT_ORDINARY_TRANSITION_BYTES_V3` 1784 → **1880** (`:231`), and the transition byte array rewritten. This is the one file on the branch that the crate actually compiles — and every red gate below is downstream of these four moved numbers. |

## 2. THE SEAMS

Ordered by hardness, because this branch's seams are not all the same kind: four
gates are **red right now**, two readers are **silently wrong**, and the family's
own machinery is **absent**.

### 2.1 RED NOW — a gate or test fails at this HEAD

| # | file:line | the one-line change |
| --- | --- | --- |
| S1 | `crates/dclutch-trading/src/ordinary_v3.rs:528` | `assert_eq!(DIRECT_ORDINARY_TRANSITION_BYTES_V3, 1_784);` → `1_880`. **`cargo test -p dclutch-trading` is red at HEAD.** Note the enclosing test is named `exact_program_admits_price_improved_ioc_and_routes_the_seller_leg_alone` (`:522`) — "price improved" is exactly the concept the equal split abolishes, so its fixture prices need re-deriving, not just the width. |
| S2 | `packages/dclutch-sdk/lib/generated/directInlineV3.ts:182` | `DIRECT_ORDINARY_COMMON_SCALARS_V3 = 68` → `71`. Fails `npm run abi:direct-v3:verify` (`packages/dclutch-sdk/package.json:39`). Fix by running `npm run abi:direct-v3`, which regenerates from `crates/dclutch-trading/src/generated_ordinary_v3.rs`. Downstream readers to re-check: `packages/dclutch-sdk/lib/directHotChain.ts:765` and its test at `directHotChain.test.ts:262`. |
| S3 | `docs/reference/abi/directInlineV3.md:250` | `\| DIRECT_ORDINARY_COMMON_SCALARS_V3 \| 68 \|` → `71`. Flows from S2 via `tools/gate reference --converge`. |
| S4 | `tools/gates/emission-coverage.md:4`, `:55`, `:154`, `:182` | `generated_intent_v3.rs:1` carries the `@generated by … EmitDirectIntentV3Rust.lean` header, so `tools/gates/emission.py` discovers it as a generated file; no guard names that emitter, so `tools/gate emission --verify` fails with a newly-unguarded file. Fix the guard **first** (S6), then `tools/gate emission --write` — writing the census first would record the unguarded file as accepted debt. |

### 2.2 SILENT DIVERGENCE — compiles green, behaves wrong

| # | file:line | the one-line change |
| --- | --- | --- |
| S5a | `crates/dclutch-operator/src/direct_inline_v3.rs:1735-1737` | The off-chain preview refuses only `execution_price < seller.limit_price \|\| > buyer.limit_price \|\| > price_scale` — the **old interval rule**. It will happily build a request at any price strictly inside the interval that the on-chain program now refuses. Replace with an equality against `(seller.limit_price + buyer.limit_price) / 2`, keeping the crossing check. |
| S5b | `packages/dclutch-sdk/lib/directInlineV3.ts:742-743` | The exact TS twin of the same stale rule. Same fix. **These two are the most dangerous lines on the branch**: nothing fails, and the operator cheerfully constructs transactions the chain rejects. |

### 2.3 MISSING MACHINERY — nothing breaks yet because nothing exists

| # | file:line | the one-line change |
| --- | --- | --- |
| S6 | `crates/dclutch-trading/check-successor-generated.sh:16,21,29,33,52,57,61` | No freshness guard exists for `generated_intent_v3.rs` (the script runs `EmitDirectIntentV2Rust.lean` at `:30` and `EmitDirectOrdinaryV3Rust.lean` at `:32`; `intent_v3` appears nowhere). Add the parallel stanza: a `generated_intent_v3=` binding, a `candidate_intent_v3` mktemp + trap, `lake build DClutchSemantics.DirectIntentV3Codec` and `lake env lean --run EmitDirectIntentV3Rust.lean`, a line-count floor and a `grep -q '^pub const COMPACT_INTENT_BYTES_V3: usize = 172;$'` pin, then `rustfmt` + `cmp`. `tools/gates/emission.py:42` matches the literal `lake env lean --run <Emitter>.lean` inside a check script, so adding the line *is* what registers the guard — there is no separate list to edit. |
| S7 | `crates/dclutch-trading/src/lib.rs:19` | Insert, after the `generated_intent_v2` block at `:16-18` and in its exact form: `#[rustfmt::skip]` / `#[allow(missing_docs)]` / `mod generated_intent_v3;`. The `allow` is required — the crate is `#![deny(missing_docs)]` at `lib.rs:3` and the emitter prints no doc comments. **Until this lands the file is not in the crate graph and its 22 constants are inert text.** See §6. |
| S8 | `crates/dclutch-trading/src/intent_v3.rs` (does not exist) | Write the Rust `CompactIntentV3` codec, copying `crates/dclutch-trading/src/intent_v2.rs` (which `pub use`s the generated constants at `:10-15` and hosts `pub struct CompactIntentV2` at `:26`), adding the counterparty field and the bearer predicate; then `pub mod intent_v3;` at `crates/dclutch-trading/src/lib.rs:37`. |
| S9 | `formal/dclutch-semantics/DClutchSemantics.lean:37` and `:45` | Insert `import DClutchSemantics.DirectIntentV3Codec` after `:36` (`DirectIntentV2Codec`) and `import DClutchSemantics.DirectRfqV1` after `:44` (`DirectRegisteredFillV4`). **This is an inventory seam, not a build break** — `lakefile.toml:11` is `globs = ["DClutchSemantics.+"]`, so both modules already compile; the root file's own header (`:1-7`) claims to be "every file under DClutchSemantics/, alphabetically", and that claim is now false. No `lean_exe` entry is owed: the lakefile comment (`:5-8`) is explicit that emitters are run as scripts by their guards and none was ever `lake exe`'d. |

### 2.4 The producer-missing seam

**There is no Rust implementation of this family at all.** Every one of
`COMPACT_INTENT_MAGIC_V3`, `COMPACT_INTENT_BYTES_V3`, `COMPACT_INTENT_VERSION_V3`,
`COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V3`, `COMPACT_INTENT_COUNTERPARTY_OFFSET_V3`
has exactly one hit in the tree: its own definition. Absent: a `CompactIntentV3`
codec (S8), an RFQ route in `programs/dclutch-trading-sbf/`, an operator builder
for a ticket pair, a TS encoder, and any test that decodes a V3 ticket.

**Name trap for the convergence lane:** `SignedDirectIntentV3` exists in ~40
files (`crates/dclutch-direct-ticket/src/envelope.rs:26`,
`crates/dclutch-operator/src/direct_inline_v3.rs:246`,
`packages/dclutch-sdk/lib/directInlineV3.ts:172`). Its `V3` is the *inline-route*
V3 and its field is `pub intent: CompactIntentV2` (`envelope.rs:33`) — no
counterparty, no bearer flag. Nothing in Rust or TS can construct, sign or read a
`CompactIntentV3`. Do not mistake the one for the other.

Also orphaned: the three `#[cfg(test)]` fixtures in `generated_intent_v3.rs`
(`:29`, `:43`, `:59`). Their V2 peers are consumed at `intent_v2.rs:336,339,383,386`;
the V3 ones by nothing. They are the ready-made assertions for S8's test module.

The SDK generator needs two rows before a browser can read a V3 ticket:
`packages/dclutch-sdk/scripts/generate-direct-inline-v3.mjs:69` (the `sources`
map has `intent: '…/generated_intent_v2.rs'` and no V3 entry) and `:411` (the
emit list ends the intent block at the V2 collateral offset).

### 2.5 Registries — mostly NOT seams, and it matters which

| registry | verdict |
| --- | --- |
| `tools/gauntlet/magic-collisions.json` | **No row.** `DCLTDIW3` is unique. The census walks every `.rs` under `crates/` and `programs/`, so it sees `generated_intent_v3.rs` even though `lib.rs` does not — one name, one value, green. |
| `docs/reference/routes.md` | **No hand row** — the file is `@generated` from the route census, and there is no Rust route to census. The seam is upstream (S8 then a dispatcher arm). |
| `docs/reference/refusals.md` | **No hand row yet, but ten codes are owed.** `Rfq.crossed`'s nine conjuncts plus `counterpartyAdmits` are documented as "one refusal by name in the adapter" — and the adapter does not exist, so zero of the ten have a `TradingSbfError` variant. |
| `tools/gates/wire-vector-pins.tsv` | **No row.** It pins two-sided Rust↔TS fixtures; no TS `CompactIntentV3` encoder exists. |
| `tools/cohort/steps.tsv:71` | **No row needed, and the existing `fill` row is safe by accident** — it authors both tickets with `--limit-price 1000000`, so floor = cap and the equal split is unchanged (`an_agreed_price_admits_unchanged`). **A future cohort with asymmetric limits breaks under the new rule.** Worth a comment on that row. |
| `tools/gauntlet/CU_BUDGETS.json` | No row names the ordinary transition directly, so nothing is mechanically red — but the program grew 73 → 77 instructions (+96 bytes) and any Direct hot CU pin drawn before this branch was measured against a shorter program. |

### 2.6 A cross-family dependency worth flagging

`DirectRfqV1.lean` names `ClearingPriceV1` in prose (`:37`, `:141`) as the
persistence surface for a derived price. That identifier does not exist on this
branch — it is defined on **`build/joint-clearing`**
(`DClutchSemantics/ClearingPriceV1Abi.lean`). The two families were built in
parallel and this is the seam between them: whichever lands second inherits the
obligation to make the reference real.

## 3. Provisional rulings

The maker recorded none — it stopped before writing its doc. These are the
rulings **its code makes silently**, surfaced so a convergence lane can reverse
them deliberately rather than discover them:

1. **The RFQ price is the floored equal split of floor and cap**
   (`DirectRfqV1.lean:51`; enforced on-chain by `DirectOrdinaryV3.lean:541-551`).
   Not seller-favouring, not taker-favouring, and explicitly not a matcher's
   proposal: `matcher_price_certifies` (`:453`) proves 55 *certifies* the book
   and `matcher_price_is_not_derived` (`:454`) refuses it anyway. **This is the
   branch's real content** and the reason S5a/S5b are divergences rather than
   omissions. Reversal cost: one `def`, four VM ops, and the four theorems.
2. **The split rounds down** (`the_equal_split_rounds_down`, `:750`): 40/61
   clears at 50, and naming 51 refuses. Reversal: the rounding direction is one
   `mulDivFloor`.
3. **An RFQ is exactly two orders and its batch is born closed**
   (`Rfq.batch`, `:205`). There is no collection phase — an RFQ never sits in a
   book. This is what makes the family "batch-spine": it reuses
   `JointClearingV1`'s machinery without reusing its lifecycle.
4. **The price vector touches the named outcome and its complement only**
   (`Rfq.priceVector`, `:143`, via `complementIndex`, `:137`). A 5-outcome RFQ
   prices two cells and zeroes three.
5. **Self-crossing is refused**, not permitted-and-netted (`crossed`'s ninth
   conjunct; `self_cross_not_crossed`, `:466`).
6. **The counterparty is on the ticket, not on the match**
   (`counterpartyAdmits`, `:217`; `bearer` is the all-zero counterparty). That is
   the whole reason V3 exists over V2, and it is why the wire grew by exactly 32
   bytes.

## 4. Stubs

**None.** No `todo!()`, `unimplemented!()`, `TODO`, `FIXME`, `XXX`, or `sorry`
anywhere on the branch. What is missing is missing by *absence* — unwritten
files, not marked holes — which is more dangerous, because nothing in the tree
points at it. §2.3 and §2.4 are the list.

## 5. Tests

**No Rust or TS test was written.** What stands in for tests is the Lean witness
corpus and the theorems over it — which prove the *model* and say nothing about
any Rust. What each would prove once S8 exists:

| witness / theorem | would prove |
| --- | --- |
| `crossed_clears` (`:335`) | the central claim: a crossed RFQ clears at the derived price. |
| `crossed` (`:418`) / `agreed` (`:427`) | the derivation at a strict cross and at floor = cap. |
| `wide` (`:435`) / `last` (`:441`) | the complement index is right away from outcome 0 and at the final outcome — the off-by-one the vector shape invites. |
| `matcher_price_certifies` + `matcher_price_is_not_derived` (`:453`,`:454`) | ruling 1: a price that *certifies* the book is still refused unless it is the derivation. |
| `uncrossed_invalid` (`:460`) | floor 60 / cap 40 does not clear. |
| `self_cross_not_crossed` (`:466`) | ruling 5. |
| `over_scale_not_crossed` (`:472`) | the scale bound refuses. |
| `partial_fill_refuses` (`:486`) | a clearing that fills less than the pair. |
| `named_admits_its_taker` / `named_refuses_a_stranger` / `bearer_admits_anyone` (`:492`–`:496`) | ruling 6, all three branches. |
| `the_equal_split_rounds_down` (`DirectOrdinaryV3.lean:750`) | ruling 2, on the actual VM program rather than the model. |

The Rust assertion that is owed and absent: nothing reads
`output[SCALAR_DERIVED_PRICE_V3]` back. The peer style is
`crates/dclutch-trading/src/ordinary_artifacts_v3.rs:580`
(`assert_eq!(scalar_output[SCALAR_EXECUTION_PRICE_V3], 50)`).

## 6. Does it parse

`cargo check -p dclutch-trading --offline` at this HEAD: **green, exit 0, zero
errors, 9.41s.**

**Read that green as "the branch breaks nothing it compiles", never as "the
branch works."** Two reasons it proves less than it looks:

1. `generated_intent_v3.rs` is not declared in `crates/dclutch-trading/src/lib.rs`
   (seam S7), so the compiler never opened it. Half the branch's Rust was not
   checked.
2. `cargo check` does not build `#[cfg(test)]`. `cargo test -p dclutch-trading`
   **is red** at `crates/dclutch-trading/src/ordinary_v3.rs:528` (seam S1).

`lake` was not run here. Both new modules do compile under the lakefile glob, so
they are not orphaned from the build — but they have not been elaborated *by
anything on this branch*, and 76 of the family's 101 theorems have never been
checked in this worktree.

## 7. STOPPED HERE — the next three steps

1. **Close the four red gates, in this order:** S1 (`1_784` → `1_880`, and
   re-derive the "price improved" fixture that the equal split has made
   meaningless), then S6 (write the `generated_intent_v3.rs` guard), then
   `npm run abi:direct-v3` for S2 and `tools/gate reference --converge` for S3,
   then `tools/gate emission --write` for S4 — the emission census last, so it
   records a guarded file rather than accepting an unguarded one.
2. **Fix the two silent divergences (S5a, S5b).** The off-chain preview and the
   SDK still admit any price inside the interval; the chain now demands the equal
   split. Until these land, every operator-built ordinary fill with asymmetric
   limits is a transaction the chain will reject, and no gate will say so. Add
   the regression the tree lacks: an operator test asserting a mid-interval price
   is refused, mirroring `a_matcher_price_off_the_equal_split_refuses`.
3. **Apply S7, re-run the emitter, and write `intent_v3.rs` (S8)** with the three
   emitted fixtures as its test module. That is the smallest piece of §2.4 that
   turns the orphan constants into something with a consumer, and it is the
   precondition for the RFQ route, the refusal codes, the operator builder and
   the TS decoder — none of which should be started before it.

Deliberately **not** steps 1–3: the SBF RFQ route, the TS encoder, and the
registry rows of §2.5. Each registers or exposes a route that does not exist yet,
and adding them first would put four more authors on a wire whose Rust author has
not been written. Also deliberately deferred: S9, the two Lean imports — they are
cosmetic (the glob already compiles the modules) and cost nothing to do last.
