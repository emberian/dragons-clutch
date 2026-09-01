# OPERATOR_FORMS_V1 — what the consoles are asking you for

**Status:** design spec + audit; vocabulary implemented, two of nine consoles converted. **Scope:** the nine
operator consoles of `apps/dclutch-web` and the shared field vocabulary they
ask their questions with.
**Written for:** the lane that converts the remaining consoles, and for anyone
adding a field to one.

This document does not re-derive FLOWFUL_IA_V1's diagnosis and does not
re-litigate its operator ruling, which stands unchanged and governs everything
below:

> **grouping, not simplification; technical precision preserved.**
> "No operator-console feature loss. Every input, every readout, every
> downloadable artifact survives. The consoles get *grouping* and *flow*; they
> do not get simplified. Precision is the feature."

FLOWFUL treated the trader's journey and left the operator's forms for a later
pass, on the grounds that the operator tier was "fine — flow helps, precision
must survive." That judgement was about **information architecture** and it was
right. This document is about the **forms**, and there the tier is not fine.
The measurements in §1 are what a full inventory found.

---

## 0. The governing rule, and where it comes from

The rule is not new. It is already written in this repository, in the doc
comment of the one field component that follows it:

> **"if a console asks you to paste something and you don't know where it comes
> from, that's a bug in the console."**
> — `components/ArtifactInput.tsx`, restated at `README.md:155`

`ArtifactInput` obeys that rule completely. It names the artifact, names its
producer and file in one concrete sentence, and — the part nothing else on the
site does — **reports what it actually received**: the byte count, the SHA-256,
and how far off the expected width it is. One field type out of 106 does this.

So this spec's rule is that one generalised, plus its converse:

> **A field must be able to say three things: what it is, where its value comes
> from, and what it just made of what you typed.** A field that cannot say the
> third is not validating — it is waiting to be told no by something further
> away.

Two corollaries, inherited from FLOWFUL and unchanged:

- **Nothing is deleted to make room.** Every raw value stays reachable. A
  derived field's derivation is shown, not hidden; an exact integer is never
  replaced by its gloss.
- **Refusals stay named, and move to the field that owns them.** A refusal may
  be re-ordered (remedy first) and relocated (to the field that caused it). It
  may never be softened, merged into a generic error, or hidden.

---

## 1. The audit

Every `<input>`, `<textarea>` and `<select>` reachable from the nine consoles
`/release /general /liquidity /direct /found /local /operate /product-v2
/workbench`, read at HEAD on 2026-08-31. Counted by **distinct value asked
for**, not by control: `/release`'s activation-cache PDA is rendered as two
`required` inputs bound to one state variable, and it is one field.

### 1.1 The headline numbers

| Measure | Count | Share |
| --- | --- | --- |
| Distinct fields across the nine consoles | **106** | — |
| …owned by a console | 96 | 91% |
| …contributed by shared chrome (`ClusterPicker`, present on all nine) | 10 | 9% |
| **Fully derivable** from data the page already holds | **45** | **42%** |
| Partly derivable | 29 | 27% |
| Derivable to some degree | 74 | **70%** |
| Not derivable — genuine operator intent | 30 | 28% |
| Collected, then never read by anything | 2 | 2% |
| **Reach an UNAUTHORED platform/library refusal** | **49** | **46%** |
| **Validate before you press the button** | **1** | **0.9%** |

The last two rows are the finding.

**46% of operator fields answer a typo with a sentence nobody on this project
wrote.** A mistyped program address on `/operate`, `/workbench`, `/found`,
`/release` or `/product-v2` produces `Invalid public key input` — web3.js's
words, from an unguarded `new PublicKey`. A malformed endpoint produces
`Invalid URL` — the URL constructor's words, from an unguarded `new URL`. On
`/product-v2` step 03 there are eight pubkey inputs whose only failure mode is
that one library string, with no field name attached; on `/found` the refusals
are index-only (`finalized raw record 4 must be canonical base58 text` — the
label on screen says `Linked basis raw`, and nothing connects the two).

The project knows how to do this correctly and does it in exactly one place:
`canonicalAddress()` in `lib/deployments.ts:161` wraps the decode and says
`{role} program is not a Solana address`. That function is the existing proof
that the other 41 pubkey fields are an oversight and not a policy.

**Exactly one field in 106 tells you anything before you submit** —
`ClusterPicker`'s deployment-import textarea, which parses on every keystroke.
Everywhere else validation is submit-time at best, so the operator learns about
a typo after an RPC round trip, from a single shared `aria-live` line that
serves every field on the form at once. `/workbench` has nine fields and one
error line. `/found` has sixteen.

### 1.2 Per console

Ranked by field count. "Derivable-but-asked" counts only the **fully** derivable.

| Console | Fields | Fully derivable, asked anyway | Unauthored refusals | Simulate / execute split |
| --- | --- | --- | --- | --- |
| `/product-v2` | 20 | **9** | 8 | compile → evaluate → compose; no send |
| `/direct` (= `/trade`) | 20 | 3 | 1 | authenticate → review; execution closed |
| `/found` | 16 | **9** | 15 | one construct button; download only |
| `/release` | 14 | 6 | 4 | plan → **gated** sign → export |
| `/operate` | 10 | 7 | 9 | preset → inspect → reacquire |
| `/workbench` | 9 | **8** | 9 | one acquire button |
| `/liquidity` | 4 | 1 | 1 | authenticate → build → sign |
| `/general` | 3 | 1 | 1 | inspect → verify |
| `/local` | **0** | — | — | one refresh button |

**No console has a submit-to-chain path.** Every one ends at bytes: a download,
an export, or a Wallet Standard signature over a transaction the browser never
sends. Three consoles say so in their own footers. This is already the ACT
shape §5 asks for, and the spec's job there is to make it *visible* rather than
to introduce it.

**No console asks for a keypair path, private key, or mnemonic.** Grepped
across all nine plus `ClusterPicker`, `WalletDirectory`, `lib/walletHandoff.ts`
and `lib/walletStandard.ts`. Signing goes through Wallet Standard, which never
exposes key material. §4 exists to keep this record, not to repair it.

### 1.3 The five defects worth naming

1. **The derive gap.** 45 fields are pure functions of values the page already
   holds, and are typed by hand anyway. The sharpest instance:
   `/workbench` and `/operate` each ask the operator to retype **six program
   addresses that `DeploymentV1.programs` already carries** — and both files
   already read that same store for the endpoint field sitting beside them.
   The second sharpest: `/product-v2` step 03's six record accounts are
   `deriveFinalizedRecordAddressesV1(registry, pinnedSchema, digest)` over
   fields already on the same form. No RPC required. The page asks for an
   answer it could compute, then checks it.

2. **The unauthored refusal.** §1.1. 49 fields.

3. **Refusals that name a record by index.** `/found`'s ten record addresses
   refuse as `finalized raw record 0..9`. The screen labels them
   `Realm raw record`, `Product Runtime V2 raw`, `Linked basis raw`. The
   mapping exists only in the reader's head.

4. **Two dead fields.** `/direct` collects `Maker Ed25519 public key` for both
   sides as `required` inputs. `CompactIntentV2Input` has no `maker` member;
   nothing reads them. `Collateral token account` is collected, reaches
   `collateralAccount`, and `previewDirectInlineV3` ignores it entirely. Four
   `required` controls, zero readers. A field that cannot be wrong is not a
   validated field — it is a field that does not exist.

5. **A scaffold that cannot pass its own validator.** `/liquidity` ships a
   prefilled route manifest with **38** fixed accounts; its inspector requires
   **39**. Filling in the shipped scaffold yields `Dealer route manifest
   requires 39 fixed accounts`, with no indication of which row is missing, and
   the visible label still reads "Hot38".

---

## 2. The typed-field vocabulary

Implemented in `components/operator/`. Two modules:

- `fieldReadings.ts` — pure, no React. Every validator is a **total function
  of the text**, returning a `FieldReadingV1`.
- `OperatorFields.tsx` — the components, which are the readings plus a DOM
  shape.

### 2.1 The reading

```ts
export type FieldReadingV1 =
  | Readonly<{ state: 'empty' }>
  | Readonly<{ state: 'resolved'; identity: string }>
  | Readonly<{ state: 'refused'; remedy: string; detail: string }>;
```

**Why a total function and not an `onBlur` handler.** This repo's component
tests render to static markup and assert on strings —
`renderToStaticMarkup` + `toContain`, with no DOM queries anywhere in 143 test
files. An event-driven validator can only be checked by simulating events, so
it would ship unverified. `lib/tradeFlowSteps.ts` states the same rule for the
flow: *"assignment done inline in JSX is assignment nobody can test."* A pure
reading means a test renders a field around a malformed value and asserts the
refusal is on the page.

Purity also settles *when* a refusal appears, with no event needed: **a field
refuses only text that is both non-empty and wrong.** An empty field is not yet
wrong. No field type here passes through an invalid spelling on the way to a
valid one — `12` en route to `1200` is already a legal reading of atoms, and
addresses, digests and JSON arrive by paste. There is no refusal to flash, so
there is nothing for a blur event to suppress.

### 2.2 The refusal grammar

Adopted wholesale from `lib/tradeFlowRefusals.ts`, which FLOW-2 landed:

> **The remedy comes first and the refusal survives whole.** `remedy` is one
> imperative sentence saying what the reader can do next; `detail` is what was
> actually found. *"A refusal that loses its own words has been turned into a
> mood."*

Rendered as `StepRefusal` renders it — two elements, in that order, each
holding one whole sentence. `components/trade/StepRefusal.tsx` explains why the
sentence may not be split: *"a sentence wrapped half in a `<span>` for emphasis
stops being findable"* by the `toContain` guards, so the guard then passes or
fails for reasons unrelated to whether the site is honest.

The grammar is enforced in `fieldReadings.test.ts` by a single helper every
refusal test routes through: the remedy is one sentence, ends in a period, and
opens with an imperative verb; the detail is present and is not a restatement.
A new field type cannot quietly ship a worse refusal.

### 2.3 The seven types

| Type | Reads | Resolves to | Refuses, characteristically |
| --- | --- | --- | --- |
| `PubkeyField` | base58, decoded to 32 bytes via the parse `lib/localSuccessor.ts` already uses | `32 bytes · 7Yk4…m3pQ`, plus the account's name when the page can `identify` it | a stray base58 character **by position and value**, naming the 0/O/I/l omission; a well-spelled string of the wrong byte width, **counted** |
| `Hex64Field` | 64 hex characters | `32 bytes · aaaaaaaa…aaaaaaaa` | **a base58 address pasted into a digest field**, saying which of the two it is holding; a stray non-hex character by position; a length, counted against 64 |
| `AtomsField` | a u64 integer, grouping separators accepted | `500000000 atoms · 500 collateral at 6 decimals` | a decimal point ("an atom does not divide"); a negative ("no wire encoding"); zero ("spends a slot and a signature, and moves nothing"); past u64, naming the bound |
| `EnumField` | one of a closed set, as a `<select>` | the choice, plus an optional per-choice gloss | a value outside the set, **listing the set** |
| `EvidenceField` | JSON, summarised by the caller | a summary **card** — `reads as` plus the caller's rows | a truncated paste ("Paste the whole file, from its opening brace to its closing one"); a well-formed document of the wrong shape, via the caller's own two-part `FieldRefusalV1` |
| `U64Field` | a plain unsigned integer, with a `noun` and optional bounds | `1400000 · 1,400,000 compute units` | a decimal; a negative; a value outside the bound, **naming both ends**. The refusal names the noun, so it is about this field rather than about arithmetic |
| `EndpointField` | a JSON-RPC endpoint | `http · 127.0.0.1:20890` | a bare host with no scheme, **showing an example**; a non-http scheme, naming the one it got |

The last two were not in this spec's first draft. They were added during the
conversion, because the audit's own type census demanded them: `u64-plain` is
the **most common numeric type** on these consoles (~15 fields, every one of
them carrying an `inputMode="numeric"` that constrains nothing), and the seven
endpoint fields between them account for seven of the 49 unauthored refusals.
A vocabulary that did not cover them would have left the largest cheap win on
the table.

Three rulings inside that table are load-bearing:

**`AtomsField` types atoms and glosses the amount — the inverse of the trade
panel.** FLOWFUL §5.4: *"Operator surfaces keep raw atoms as the primary
rendering … their readers are checking arithmetic against a chain, and a
thousands separator in a byte offset is a hazard."* So the exact integer is
what is typed and what is stored, and `formatQuantityV1`'s humanized amount is
what the field reports back. The gloss is never the value. When the mint
published no display precision the field says exactly that rather than
inventing a scale, and never invents a ticker — the unit is the word
`collateral`, per `UNNAMED_COLLATERAL_UNIT_V1`.

**`U64Field` and `AtomsField` are different types on purpose.** Atoms are a
*quantity* of something a mint denominates and carry a display precision;
generations, compute limits, scalar identities and outcome indices are counts
and identifiers, which have a noun but no decimals. Serving both with one field
would mean either inventing a scale for a generation or dropping the gloss from
an amount. So the reading names the noun in one case and the denomination in
the other.

**`PubkeyField` carries no canonical-spelling refusal, and that is measured.**
base58's encoding of a *fixed* 32-byte width is injective, so any string that
decodes to 32 bytes is already the only spelling of them. (`localSuccessor.ts`
and `deployments.ts` do carry round-trip checks, correctly — they decode values
out of JSON, where the input may not be an address at all. A field bound to a
text input has already been narrowed.) A branch that cannot fire would be a
test that passes vacuously, so instead the **property** is pinned: the test
proves the round trip holds and that the obvious second spelling does not
decode. If a future decoder swap weakens it, that test fails.

### 2.4 What the components add

The DOM is deliberately the shape the stylesheet already expects — a wrapping
`<label>` with a bare `<span>` inside it — so a typed field dropped into a
`.direct-card` or `.trade-v3-card` inherits that card's control styling
unchanged. The only new CSS is the reading line, and it borrows established
colours: `.artifact-fact`'s muted mono for a reading, and the refusal amber
`--color-destructive` that `.direct-refusal` has always used.

Four parts, in order: **label · control · provenance · reading.**

---

## 3. The DERIVE rule

> **A field derivable from the chain or from another field pre-fills from that
> read, shows its provenance, and stays editable where overriding is
> legitimate. A field that is a *pure function* of other fields on the same
> form is not asked for at all — it is computed and displayed.**

The authority is WAVE.md's canonical-generation mandate (ember, 2026-08-31):

> "an expectation is either (a) DERIVED from chain state and verified for
> internal consistency the way the chain verifies it, (b) GENERATED from the
> single Lean/Rust author with a byte-identity gate, or (c) one of the
> irreducible roots … **Hand-carried pins are a defect class, not a style
> choice.**"

A console field whose value the page has already read is a hand-carried pin
with a text cursor in it. There are 45 of them.

### 3.1 The three states, and why editable

`DerivedProvenance` renders exactly three sentences, generalising the two
`/release` already writes by hand:

| Situation | What the line says |
| --- | --- |
| derived, and the box matches | **Filled from {source}.** You can paste a different value; this line will say so. |
| derived, and the box differs | Manually set. {source} reads {compact}. |
| nothing to derive from | *the console's own sentence, which must name the way to get it* |

Overriding stays available because **the operator is frequently the person who
knows the read is stale** — that is much of what an operator is for. What
changes is that overriding becomes a *visible act* rather than an
indistinguishable one. The third state is a requirement, not a fallback: a
console that cannot derive a value must still say where the value comes from,
which is the `ArtifactInput` rule again.

### 3.2 When to compute instead of ask

`DerivedValue` — shown, not asked — when **all** of these hold:

1. the value is a pure function of fields already on the form (no new RPC);
2. a disagreeing paste would be refused anyway, so the input can only
   introduce an error the console then has to detect;
3. the derivation can be named in one concrete sentence.

`/product-v2`'s six record accounts are the clean case: `registry` + a pinned
schema id + a digest already on the form. The console today asks for all six,
then refuses any mismatch. Computing them removes six inputs, eight unauthored
refusal paths, and an entire class of mistake — and removes no fact, because
the computed address is displayed and copyable.

**Where a derive would need a new chain read, it does not happen in this pass —
it gets recorded instead.** `/found`'s five cross-derivable records
(result-domain and portfolio from the Product record; source-spec,
capacity-profile and manipulation-floor from `SourceMaterialV3`) are derivable
only by fetching and decoding those accounts, which `/found` does not do before
submit. The joins are already *verified* server-side at `coreFound.ts:435` and
`:676-678` — the console checks an answer it could have computed. Closing that
is a real improvement and a new RPC dependency, so it is named here as queued
work rather than smuggled in under a forms pass.

---

## 4. The KEYPAIR rule

> **No key path is ever typed into a browser.** A field asking for
> `~/.config/solana/id.json` asks the reader to name secret material to a web
> page, and it cannot even work — the browser has no filesystem to read it
> from, so the value is decorative right up until someone believes it is not.
>
> What replaces it is not a smaller input. It is the CLI invocation that does
> the signing, with the environment variable named, so the act stays where the
> key already lives.

**The audit found zero violations.** This is the one thing the consoles get
unambiguously right, and `KeypairHandoff` exists to keep it that way: it is the
shape the next console reaches for when it needs an offline signature, so that
"ask for the path" never becomes the obvious move. Its test asserts the
component renders no `<input>` at all.

The rule's positive form is already on `/release`, whose helper text reads
*"paste an address whose keypair you hold elsewhere"* — an address is public,
and naming where the key is *not* is part of saying what the field wants.

---

## 5. The ACT shape

> **Simulate is the primary act. Execute is gated behind a green preflight, and
> the two are never one button.**

This is CLOSE-DRIVER's dry-run-as-property rendered as UI, and FLOWFUL's ninth
prohibition — *"Do not collapse sign into send. Two acts, two buttons, two
states, always — even when it costs a click. Especially then"* — applied to the
consoles.

The consoles already satisfy the hard half: **none of them can submit.** What
they lack is a visible relationship between the buttons. `/release` is the
existing proof of the right shape, and it should be the model: `releaseUngateV1`
opens signing only when a plan is green against this chain **and** the connected
wallet is exactly that plan's declared fee payer, and it says so in a full
sentence when it refuses:

> `No activation plan is green against this chain. Signing stays closed. It
> opens only when one activation plan is green against this chain and the
> connected wallet is exactly that plan's declared fee payer.`

That is the pattern: **a closed gate states its own opening condition.** A
disabled button that cannot say why is the flat-console failure in miniature —
`FlowRail`'s doc comment makes the same point and its tests forbid the word
"greyed-out".

**The act fieldset.** Grouping follows the acts, not the data types: the fields
one button consumes are one `<fieldset class="operator-act">` under a legend
naming the act and a sentence naming the question it answers. This is grouping
in FLOWFUL's sense — no field moves out of reach, no field is removed, and the
raw values stay exactly where they were.

---

## 6. The refusal-at-owning-field rule

> **A refusal is rendered at the field that caused it. A form-level line is for
> refusals that no single field owns.**

Today every console routes every refusal through one shared `aria-live` line —
`Refused: ${message}` — so a sixteen-field form has one error slot. FLOW-2
solved the identical problem for the trade flow with a routing table:
`assignRefusalV1(detail, fallback)` matches a fragment of the message the
machine already produced and returns the step that owns it, plus a remedy,
leaving the original words intact. Its stated reason applies verbatim here:
rendering everything under one heading *"tells a reader that something is wrong
somewhere behind them."*

The console analogue is a table from a refusal fragment to the **field** that
owns it. `components/operator/foundRefusals.ts` is the first one, built for
`/found` because `/found` needed it most: sixteen fields, one `aria-live` line,
and ten refusals that name their record by *position* while the screen names it
by *role*.

Two things make such a table cheap to build correctly:

- The audit in §1 already recorded, for all 106 fields, the exact refusal
  string and its source location. That table *is* the routing table's input.
- `lib/generated/refusalRegistryV1.ts` gives every chain-side refusal a code, a
  band and a meaning, generated from the Rust author. A chain refusal that
  reaches the browser already carries a code; what it lacks is the field on
  *this form* that produced it.

Three rules on top:

1. **Never paraphrase the detail.** Route it and prepend a remedy.
2. **An unrouted refusal keeps its place and says so**, exactly as
   `assignRefusalV1` does with `routed: false` — an unmatched fragment renders
   at form level with the fallback remedy, never silently attributed to a
   guess.
3. **The table must be verified against the source, not against memory.**
   `foundRefusals.test.ts` reads `lib/coreFound.ts`, `lib/infrastructure.ts`
   and `lib/rpc.ts` and asserts every routed fragment really exists there —
   decomposing `${field} <template>` refusals and requiring *both* halves, and
   separately asserting the ten records appear in `rawAddresses` in the order
   the table assumes. This is not ceremony: on its first run it caught a real
   error, an entry carrying `/release`'s `activation cache has the wrong owner`
   wording for a `/found` field whose actual refusal says `wrong Registry
   owner`. An unverified routing table is fiction that renders.
4. **A field-level reading pre-empts a form-level refusal.** If the field can
   tell you the address is malformed before you submit, the submit-time refusal
   for that malformation should be unreachable. Where it becomes unreachable,
   that is a *reduction in what the chain has to catch*, and the guard tests
   should pin the field-level string rather than deleting the old assertion.

---

## 7. Conversion order

Ranked by (fields × defect density), skipping `/direct`: it is a byte-for-byte
duplicate of `/trade` that FLOWFUL already rules should become a redirect, and
FLOW-2 owns that surface.

| Order | Console | Why it is first | Status |
| --- | --- | --- | --- |
| 1 | `/product-v2` | 20 fields, the largest genuine console; 9 fully derivable, **6 of them pure functions of other fields on the same form** — the flagship derive, needing no RPC; 8 unlabeled pubkey refusals; the only console mixing hex64 and pubkey fields, which is the confusion `Hex64Field` exists to catch | **converted** |
| 2 | `/found` | 16 fields, 9 fully derivable, 15 unauthored refusals, and refusals that name records by index while the screen names them by role | **converted** |
| 3 | `/workbench`, `/operate` | 6 program fields each, ~100% derivable from a store the file already reads for the field beside them. Small, and the highest ratio in the audit | queued |
| 4 | `/release` | already the most evolved — `ArtifactInput` and hand-written provenance lines are the seeds this vocabulary grew from. Mostly a port | queued |
| 5 | `/liquidity`, `/general` | 4 and 3 fields; `/liquidity` also needs the 38-vs-39 scaffold defect fixed | queued |
| — | `/local` | zero fields. Its real defect is that it renders a cluster `<select>` whose selection it then ignores, being pinned to the checkpoint's URL | out of scope |

### 7.1 What the two conversions actually changed

**`/product-v2`.** Six `required` inputs became six derived values, computed by
`deriveProductV2AccountsV1` from the Registry program, three pinned schema ids
and the three digests already on the form — no RPC added. Eight unlabeled
`Invalid public key input` paths are unreachable from the main flow. The three
digests became `Hex64Field`s, which is what now catches an address pasted into
a digest box; the five step-01 scalars became `U64Field`s. Overriding a derived
account survives behind one `<details>`, and the override says whether it is
set. Its test file went from 1 assertion block to 11.

**`/found`.** All fourteen addresses gained a provenance sentence — the console
previously offered a label and nothing else. Two fields that were *already*
silently pre-filled from the deployment now say so. Sixteen fields were grouped
into four act fieldsets. The endpoint became an `EndpointField` and the
generation a `U64Field`, closing two unauthored refusal paths. And
`assignFoundRefusalV1` routes 27 refusal fragments to the field that owns them,
turning `finalized raw record 4` into a remedy at the field labelled
*Linked basis raw* — while three join refusals stay deliberately unrouted,
because the field that can fix them depends on which side of the join is wrong.

Neither conversion removed an input, a readout, or an artifact. The one input
count that fell — `/product-v2`'s six — fell because those values are now
computed and displayed, and remain overridable one click down.

---

## 8. What NOT to do

1. **Do not simplify.** FLOWFUL's operator ruling governs. Every input,
   readout and artifact survives; a computed value is *displayed*, never
   dropped.
2. **Do not let a gloss become the value.** `AtomsField` stores atoms. The
   humanized amount is a reading, never the state.
3. **Do not invent a token symbol.** Absent an editorial entry the unit is the
   word `collateral`.
4. **Do not make a guard vacuous.** The `not.toContain` guards are a ratchet.
   If a conversion makes one pass for a new reason, pin the new string; do not
   weaken the assertion. Keep the `<details>` drawers — a drawer is depth, and
   depth is how precision survives grouping.
5. **Do not add an RPC read under a forms pass.** Derives that need a new chain
   fetch get recorded (§3.2), not smuggled.
6. **Do not paraphrase a protocol refusal.** Route it, prepend a remedy, leave
   its words whole.
7. **Do not disable a button without saying why.** A closed gate states its
   opening condition, in a sentence.
8. **Do not ask for a key path.** §4. Not even "for local development".
