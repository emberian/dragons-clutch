# Seam audit — every inter-program identity pin, 2026-08-29

One question, applied to every seam the protocol has:

> **Which identities does the other side of this seam pin, and does this side agree?**

It paid four times in one lane on Dealer↔Custody and Dealer↔Claims earlier today
(`DEALER_ACCEPTED_TRANSITION_2026_08_29.md`). This document applies it to the
seams that had not been asked: Trading↔Claims, Trading↔Resolution,
Claims↔Custody, Core↔Registry, Core↔Trading, and the Token-2022 boundary — plus
a repo-wide sweep of the two mechanical defect classes.

Method was static two-sided comparison first: enumerate what side A pins (exact
keys, seed tuples, censuses, prestate demands), enumerate what side B builds or
requires, diff them, and only then drive a machine to confirm. Every claim below
carries a `file:line`. Verdicts are one of **always-refuses** (no input satisfies
both sides — the route is dead), **always-admits** (the mirror image: a required
identity is not pinned at all), **latent** (satisfiable today, but the two sides
can drift apart with nothing to catch it), or **benign**.

---

## Ranked findings

| # | seam | verdict | what disagrees |
|---|---|---|---|
| [1](#1) | Claims internal (SignedDeltaV3) | **always-admits** — *fixed, `fb4b5ad8`* | `CallerRole::Core` authenticated nothing at coordinates 14/15 while the route's authority was derived from coordinate 14 |
| [2](#2) | Token-2022 writer ↔ readers | **always-refuses** — *verdict: the writer is wrong* | the protocol writes a 202-byte mint; every reader requires ≥238 with an extension nothing initializes — and the terminal path really does burn through it |
| [3](#3) | Trading ↔ Registry | **always-refuses** — *fixed, `9a9f1b5c`* | Trading authenticated ten Registry records with a 2-seed derivation; the Registry creates them with 3 |
| [12](#12) | Core ↔ Trading (capability) | **always-refuses** — *fixed, `3b98ea3a`* | `ActivateCapability` ran a global no-duplicate census over a frame that structurally requires seven repeats |
| [13](#13) | Core ↔ Trading (founding) | **always-refuses** ×3 | `5ca145e8` made the funding source the fee payer; three consumers that pin it *by identity* did not follow |
| [4](#4) | Token-2022 operator ↔ program | **always-refuses** | the wallet-facing builder pins a mint at exactly 82 bytes that the program requires to be ≥238 |
| [5](#5) | Token-2022 ATA ↔ account reader | **always-refuses** | builders derive canonical Token-2022 ATAs (170 bytes); the reader gate is exactly 165 |
| [6](#6) | Core ↔ Resolution (harness) | **always-refuses** | the gauntlet journey still asserts the poststate `da5460b3` removed |
| [7](#7) | Claims ↔ Custody (published ABI) | **always-refuses** | the ABI page drops the one coordinate a builder cannot reconstruct |
| [8](#8) | Structured V2 seed orders | latent — *fixed, `fb076ec6`* | two PDA seed domains over Solana's 32-byte maximum |
| [9](#9) | Claims ↔ Custody | latent | Claims pins nothing about the Custody program's identity on the replay leg |
| [10](#10) | Claims ↔ Custody | latent | the payout leg's 14-account frame has no compile-time tie to Custody's declared width |
| [11](#11) | assorted | latent / benign | see [Latent](#latent-hazards) and [Benign](#benign-verified) |

**Nine always-refuses routes and one always-admits, across six seams, none of
which had a failing test.** Four were fixed in this lane (`fb076ec6`,
`9a9f1b5c`, `fb4b5ad8`, `3b98ea3a`); the rest are posted to their owners with
probes.

Two seams came back **clean on their own terms** and that is worth stating
plainly rather than padding: the **Core↔Resolution provider frame** after
`da5460b3` (every account table, census, and caller-PDA seed tuple agrees at
every one of six derivation sites — the defect is one layer out, in a harness),
and the **Claims↔Custody programs themselves** (all four seed tuples match
segment for segment across four independent authors, and the phase pair that
would have made payout dead is explicitly absent).

---

<a name="1"></a>
## 1. `SignedDeltaV3` authenticates nothing for a `Core` caller — always-admits

**The pin that is a comment instead of a check.**
`/Users/ember/dev/dclutch/programs/dclutch-claims-sbf/src/signed_delta_v3.rs:563-608`
authenticates the caller's program/programdata at coordinates 14/15 for two of
the three roles:

- `CallerRole::Claims` — explicit pin, `caller_program == claims_program` (`:575-580`).
- `CallerRole::Trading` — authenticated against the Registry activation cache
  (`caller_is_trading.then_some(...)`, `:595-599`).
- `CallerRole::Core` — **nothing.**

The comment at `:568-573` asserts the missing case is already covered — *"a Core
caller passes its own program there and is already covered by the first entry"* —
but the first entry authenticates `accounts.core_program`, a different
coordinate. Nothing requires `caller_program == core_program`.

**Why that coordinate is load-bearing.** `authenticate_authority` (`:470-497`)
derives the whole route's authority under exactly that unauthenticated account:

```rust
if accounts.authority.key
    != &Pubkey::find_program_address(&seeds.as_slices(), accounts.caller_program.key).0
```

It refuses only `CallerRole::Claims` (`:481-484`). So for role `Core` the
authority is a PDA under whatever program sits at coordinate 14, and that
program is never checked against anything.

**The fix is already written, three files away.** The sibling route implements
precisely the missing arm —
`/Users/ember/dev/dclutch/programs/dclutch-claims-sbf/src/rational_representation_v2.rs:674-680`:

```rust
ExecutionRoleV1::Core => {
    if base.caller_program.key != base.core_program.key
        || base.caller_programdata.key != base.core_programdata.key
    { return Err(ClaimsSbfError::Release.into()); }
}
```

The other two siblings authenticate the caller coordinate *unconditionally*
against `execution_role(caller_role)`:
`sparse_native_transfer_v1.rs:388-396` and `affine_batch_v2.rs:456-464`.
`signed_delta_v3` is the only one of the four that special-cases the role, and
the special case is the hole.

**Reachability.** `SignedDeltaV3` is magic-selected from the top-level dispatcher
(`programs/dclutch-claims-sbf/src/lib.rs:288-291`) and `decode_role` admits
`Core = 0` (`crates/dclutch-claims-svm/src/signed_delta_v3.rs:1237-1242`). The
role is also produced internally: `rational_terminal_v3.rs:621-624` maps
`CallerRoleV2::Core → CallerRole::Core` into `execute_parent_signed_delta_v3`.

**Why nothing went red.** Nothing in the tree *builds* a `SignedDeltaV3` with
`caller_role: Core` — every constructed plan uses `Trading`
(`crates/dclutch-claims-svm/src/signed_delta_v3.rs:1359`,
`programs/dclutch-claims-sbf/program-test/sparse-chain/tests/sparse_chain.rs:554`,
`.../fractional-atomic/tests/fractional_atomic.rs:1994,2070`,
`.../affine-batch/tests/affine_batch_v2.rs:350,489`), and every builder resolves
coordinate 14 to Trading. Decodable-but-unbuilt surface with no guard: the tests
cannot fail because no test asks the question.

**Cheapest probe, one byte on an existing ProgramTest.**
`/Users/ember/dev/dclutch/programs/dclutch-claims-sbf/program-test/fractional-signed-delta/tests/fractional_signed_delta.rs`
already places `TEST_CALLER_PROGRAM_ID` at coordinates 14/15 (`:474-475`) and
derives the authority under it (`:432-441`), while activating it as `Trading`
only (`:200,216,220-227`). Flip the plan's role byte and the seeds' role from
`Trading` to `Core` and leave 14/15 alone: today that transaction is accepted;
it must refuse `SignedDeltaSbfErrorV3::Release`.

**Fixed in `fb4b5ad8`** (authorized after the finding was posted; Claims is
unowned since FRAC closed). The caller coordinate now has a **type** rather than
another `if`: `CallerCoordinateV3` says, per role, which coordinate must hold the
caller program — its own (`Trading`, the only role that brings an external
program), or Core's, or Claims'. There is no variant meaning "unpinned and
unauthenticated", which is what `Core` silently was, and a fourth role cannot be
added without choosing one.

The test derives its expectation from the **authority** side — `execution_role`,
the role that actually enters the seeds — and never from the pin under test, so
it cannot agree with a wrong pin by construction. It reads as one sentence:
*whatever program the authority is derived under must be a program the Registry
authenticated for exactly the role in that authority's seeds.*

**Control.** Modelling the pre-fix mapping with a fourth `Unpinned` variant, the
new assertion fails for `Core` and still passes for `Claims` and `Trading` — one
failing branch, and the right one. No live route changes behaviour: nothing
builds a `Core`-role plan, so this closes decodable-but-unbuilt surface.
`cargo test -p dclutch-claims-sbf --lib` → 32 passed, 0 failed.

---

<a name="2"></a>
## 2. Every reader of the protocol's own Token-2022 mint refuses what the protocol writes — always-refuses

Four pins on one account, and no byte string satisfies more than one.

**The writer.** `rational_lifecycle_v2` allocates receipt and shard mints at
exactly `TOKEN_2022_CLOSEABLE_MINT_BYTES_V2 = 202`
(`programs/dclutch-claims-sbf/src/rational_lifecycle_v2.rs:676-683`, `:720-731`;
const at `crates/dclutch-token-svm/src/closeable_mint.rs:13`) and initializes
them with exactly two instructions —
`initialize_mint_close_authority` and `initialize_mint2`
(`rational_lifecycle_v2.rs:1046-1067`). **`PermissionedBurn` is never
initialized.**

Verified repo-wide: the only `permissioned_burn::instruction::initialize` call in
the entire tree is in a unit test
(`crates/dclutch-token-svm/program-test/tests/behavior_v2.rs:101`). No on-chain
program ever initializes that extension.

**The readers.** `Token2022BehaviorProfileV2::check_mint` ends
(`crates/dclutch-token-svm/src/behavior_profile_v2.rs:231`):

```rust
if !close_seen || !burn_seen || pointer_seen != metadata_seen {
    return Err(Error::InvalidExtensionLayout);
}
```

A 202-byte mint carrying one `MintCloseAuthority` TLV yields
`close_seen = true, burn_seen = false` → refused. That gate is what
`rational_representation_v2.rs:1836`, `fractional_atomic_v3.rs:296` and
`fractional_retirement_v3.rs:203` all read those same PDAs through — same seeds,
`RATIONAL_SHARD_MINT_SEED_V2 ‖ descriptor ‖ outcome`
(`rational_representation_v2.rs:977-981` vs `rational_lifecycle_v2.rs:714-718`).
The TypeScript mirror reproduces the same requirement faithfully
(`packages/dclutch-sdk/lib/rationalTokenV2.ts:253`).

**The repo already contains the proof, filed as reassurance.**
`programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs:3657`
(`a_receipt_mint_missing_its_burn_role_refuses_at_the_first_issue`) builds
`vec![0; 166]` plus one 36-byte TLV — **exactly 202 bytes, exactly the
lifecycle's output** — and asserts refusal. Its doc comment frames this as
*"MEASURED: that cannot happen"*. It is in fact an executable proof that the
founding path the protocol ships can never issue.

**Why the join is untested.** The two campaigns are disjoint: the
`claims-rational-lifecycle` gauntlet runs Activate/Retire against the real v11
ELF and never issues; `claims-rational-representation-v2` issues but installs its
mints as hand-planted `add_account` bytes carrying a fabricated `PermissionedBurn`
TLV (`rational_representation_v2_program_test.rs:1087-1089`) — a shape no dClutch
code path produces. Same fabrication at
`programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/fractional_atomic.rs:315-330`.

This is defect class 5 in its purest form: **each side's fixture invents exactly
the bytes that side expects, so both sides are green and the composition is
dead.**

**Verdict: the readers are protocol-correct and the writer is wrong.** The
question "which side is right" is settled by what the terminal path actually
needs, not by taste — **the protocol really does burn through the extension.**
Three on-chain sites emit `permissioned_burn_instruction::burn_checked`:
`fractional_atomic_v3.rs:958` and `:1003` (`WholeUnwrap`), and
`rational_representation_v2.rs:1364` (`BurnReceipt` / `BurnShard`). A permissioned
burn is impossible on a mint that never initialized `PermissionedBurn`, so a
reader demanding it demands exactly what the route requires. The protocol says so
in its own committed behaviour preimage too
(`behavior_profile_v2.rs:27`): `mint-required=MintCloseAuthority+PermissionedBurn`,
`instructions=…,permissioned-burn-checked,…`. And the authority passed to the burn
is `base.representation_authority` = PDA of
`(RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, descriptor)` — the two-role authority
the Structured seeds module describes.

So `initialize_closeable_mint` builds a mint its own terminal path cannot burn.

**Not fixed here, and the reason is a real one rather than caution.** The
writer-side change is a parser rewrite plus a wire-visible economic change:
`crates/dclutch-token-svm/src/closeable_mint.rs` is written for exactly one TLV at
hardcoded offsets (`TLV_TYPE_OFFSET` 166 / length 168 / value 170, exact-202
length check at `:62`, module doc at `:3-6`), so a second TLV means new layout
constants, a second parse and rebuilt tests; and 202 → 238 moves rent, because
`TOKEN_2022_CLOSEABLE_MINT_BYTES_V2` feeds `rent.minimum_balance` at
`rational_lifecycle_v2.rs:663` and `:697`, which are compared **for exact
equality** against `header.receipt_rent_principal` and `row.shard_rent_principal`
— wire fields. Four `authenticate_closeable_mint` call sites, two allocation
sites, and the program-test's own rent computation
(`program-test/rational-lifecycle/tests/lifecycle.rs:531`) follow. It also needs
the lifecycle campaign re-run against the real v11 ELF to be worth anything —
which is precisely the join that has never been tested.

**Blast radius, scoped — and it does not reach founding.** Extensions are
init-time-only, so a mint written 202-byte is broken permanently and no upgrade
repairs it. That makes "which mints" the load-bearing question, and the answer is
narrow:

- `initialize_closeable_mint` creates **exactly two families** — the receipt mint
  (`rational_lifecycle_v2.rs:671-683`) and the shard mints (`:715-731`), both
  Claims PDAs, both reachable only through `claims/lib.rs:349 →
  rational_lifecycle_v2::process`. **The collateral mint is not among them.**
- **The flagship founding ladder never goes there.** No reference to
  `rational_lifecycle`, `ActivateReceipt` or `activate_coordinate` exists anywhere
  under `tools/local-validator/bootstrap/successor/src/`, `tools/devnet-activity/`
  or `tools/release/`. Its collateral mint is an ordinary externally-keyed
  Token-2022 mint from the bootstrap forge (`market.rs:2129`), governed by
  `CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer()`
  (`market.rs:3036`) whose storage policy is
  `ExtensionStoragePolicy::ExactBaseWidthsOnly` (`token-svm/release.rs:198`) —
  zero extensions by declaration. The fresh mint each staged attempt creates
  cannot be written broken.
- **The Direct family does not need the extension.** It creates no mint (no
  `initialize_mint`/`initialize_mint2` anywhere in `src/direct/` or
  `direct_token_setup_v1.rs`) and burns nothing — a burn sweep across the whole
  Direct family and `dclutch-direct-codec` returns one hit,
  `successor.rs:1966`, and it is prose about Sell records burning "one complete
  set", not a token instruction. Direct authenticates collateral through the same
  zero-extension profile (`direct_token_setup_v1.rs:520-545`) and moves value by
  transfer.

So the defect is confined to the rational/structured **representation** family.
No permanent debt has been minted yet, because nothing under `tools/` drives that
route — it accrues the first time a campaign does.

**The fix is smaller than first stated, and the correction matters.** The
representation readers need no change at all:
`behavior_profile_v2::check_mint` **already** demands both TLVs, so a 238-byte
writer agrees with them immediately — that half is forward-compatible, not
breaking. The real coupling is only the lifecycle's *own* post-create reader,
`closeable_mint::check_mint`, which requires exactly 202
(`closeable_mint.rs:62`) and is called at `rational_lifecycle_v2.rs:683`, `:758`,
`:773` and `:801`. Writer plus `closeable_mint.rs` plus the two rent principals
move together; nothing else does.

**Closing gate for whoever takes it.** One test in the existing
`rational-lifecycle` program-test feeding the post-`ActivateReceipt` mint bytes to
`Token2022BehaviorProfileV2::check_mint`. That campaign already loads and
digest-checks the real v11 ELF. It fails today.

**RESOLVED 2026-08-29 (`f7c960b9`, `bb625688`) — see
`docs/evidence/TOKEN_2022_MINT_EXTENSION_2026_08_29.md`.** The writer issues
`permissioned_burn::initialize` and allocates 238 bytes; `closeable_mint` walks
real TLV storage through the same shared parser `behavior_profile_v2` uses; the
closing gate above is `assert_lifecycle_mint_is_terminally_burnable` and it now
passes on real post-CPI bytes. The rent coupling was smaller than sized here:
the tree pins no rent lamport figure for this account, so all three principals
moved with the width without a value being restated. §4 below is this same
defect from the wallet side and is **not** fixed — it now has an exact width to
disagree with.

---

<a name="3"></a>
## 3. Trading authenticates Registry records with two seeds; the Registry creates them with three — always-refuses

**The contradiction, and it is one program disagreeing with itself.**

- `programs/dclutch-trading-sbf/src/admitted_composition_v3.rs:587-593` —
  `require_record_key` derives `find_program_address(&[domain, identity], registry)`,
  **two** seeds.
- `programs/dclutch-trading-sbf/src/hot_v3.rs:10283` — the same program derives
  the same record family with **three**,
  `[RAW_RECORD_PDA_SEED_V1, &schema, &digest, &[raw_bump]]`.

**Which one is right — the three-seed one, and the other side pins it
unambiguously:**

- `crates/dclutch-record-contract/src/lib.rs:29` — *"PDA seed domain for the one
  raw account keyed by schema/release **and digest**"*.
- `crates/dclutch-record-contract/src/lib.rs:272-279` — `raw_record_pda_seeds()`
  returns *"the **three** exact raw-record PDA seed components"*.
- `programs/dclutch-registry-sbf/src/record_v1.rs:537-551` — `derive_record_pda`,
  the **only** derivation in the Registry program, for raw and staging alike.
- `programs/dclutch-registry-sbf/src/lib.rs:466` — same three seeds.

107 sites across the repo use the three-seed spelling. The two-seed spelling
exists at exactly one place.

**Blast radius: ten checks on the live Hot admitted-AOT route.**
`admitted_composition_v3.rs` lines 515, 521, 527, 533, 539, 545, 551, 557, 563,
569 — capability, strategy, certificate, admission and artifact records, each raw
and staging — inside `validate_authenticated_frame:451`, reached from
`pub execute_admitted_aot_v3:172`, called by `hot_v3.rs:4476`
(`execute_admitted_candidate_v3`), whose frame is built at `hot_v3.rs:4572-4592`
from `view.frame.descriptor_raw` / `strategy_raw` / … — **the very accounts
`hot_v3` already authenticated the three-seed way.** The same account is checked
both ways inside one transaction.

**Why no input satisfies both.** Solana concatenates seed segments before
hashing. The three-seed form commits 64 bytes of identity material
(`schema ‖ digest`); the two-seed form commits 32 (`identity`). Different
preimages. Line 525 makes it plainest: it passes `CAPABILITY_SCHEMA_ID_V3` as the
whole identity for a **staging cursor**, so the digest seed is simply absent.

**Why it never went red (class 5 again).** The sole test of `require_record_key`
(`admitted_composition_v3.rs:785-800`) derives its own "canonical" address *with
the function's own two-seed spelling* and then asserts the function accepts it.
A tautology: it holds for any spelling and is blind to the one thing that
matters.

**Control run:** `cargo test -p dclutch-trading-sbf --lib admitted_composition_v3`
→ **4 passed, 0 failed**, including that test. Green is not evidence here.

**A second, independent defect in the same ten lines.** Line 525 passed
`v3::SCHEMA_RELEASE_ID` (`0x0e33b25f…`) as the capability staging identity, for a
record the *same execution* had already required to be `PROGRAM_SCHEMA_ID_V4`
(`0x2d85b221…`) at `hot_v3.rs:1885`. A seed-count fix that kept that constant
would still always refuse.

**The shape of the mistake.** The ten calls were splitting one identity in half:
the raw call passed the digest, the staging call passed the schema, and neither
had a whole one. A Registry record and its staging cursor are one
`(schema, digest)` under two domains —
`crates/dclutch-record-contract/src/lib.rs:272-288`, and the same program's own
correct authenticator derives both from one pair
(`programs/dclutch-trading-sbf/src/execution_strategy_v2.rs:722-757`).

**Fixed in `9a9f1b5c`** (authorized by the coordinator; the file was clean of
other lanes' edits). The ten calls became five, each taking the whole pair, with
seeds from `RecordKeyV1::raw_record_pda_seeds()` /
`staging_cursor_pda_seeds()` rather than re-spelled — so halving an identity is
now unexpressible rather than merely corrected. Registry-truth per family was
taken from `execution_strategy_v2`'s own call sites, not from symmetry:
capability `= CAPABILITY_PROGRAM_SCHEMA_ID_V4 + capability_program_id`, strategy
`= EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2 + strategy_program_id`, certificate,
admission and artifact likewise. The test now derives through the contract and
asserts what matters: that the three-seed address is *not* the two-seed one, that
schema and digest do not commute, and that a mismatched pair refuses.
`cargo test -p dclutch-trading-sbf --lib` → **339 passed, 0 failed**;
`cargo check -p dclutch-trading-sbf --all-targets` clean.

**Nothing went red end to end, and that is the finding, not its absence.** No
e2e test drives the admitted-AOT lane at all:
`programs/dclutch-dealer-accelerator-sbf/program-test/tests/frontier.rs:775-781`
calls itself *"a frontier marker, not an acceptance test"*, and `accepted.rs:3-7`
records that the admitted Hot instruction *"resolves 121 account locks against a
64-lock runtime ceiling, so it can never be submitted anywhere"* and submits a
different route instead. This gate had never run.

**Still open, deliberately left to Trading's owner:** `require_record_pair`
checks only *keys*. `execution_strategy_v2::authenticate_finalized_record`
additionally pins `raw.owner == registry`, privileges, the content digest and
rent-exemption, and the staging cursor's System ownership and zero length. For
the certificate, admission and artifact records nothing else in this route binds
those accounts to anything real. Folding this call into that function would
delete the duplicate outright.

**Predecessor.** `ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4` was a stale literal
`38` that closed this same lane from the other end
(`admitted_composition_v3.rs:53-65`). `require_record_key` was the next gate on
the lane that fix reopened — and it had never been reached before.

---

<a name="4"></a>
## 4. The wallet-facing builder pins a mint at 82 bytes the program requires to be ≥238 — always-refuses

`crates/dclutch-rational-representation-v2-operator/src/lib.rs:1151` calls
`Mint::parse(observed.data)` on what its own doc calls *"Complete observed account
data"* (`:112-122`). `Mint::parse` refuses anything but exactly 82 bytes
(`crates/dclutch-token-svm/src/state.rs:119-121`). The on-chain reader for the
same account requires ≥166 with two mandatory TLVs, i.e. ≥238. **82 and ≥166 are
disjoint.**

This is the builder behind `construct_denominate`, `construct_reconstitute`,
`construct_issue_structured`, `construct_unwrap_structured` and
`construct_redeem_terminal` (`:362-397`), consumed by
`crates/dclutch-bearer-v2-operator/src/lib.rs:153,166,178`. Every one refuses
before it can build a transaction.

**Why it survives:** the operator's own tests build *legacy*-layout mints —
`tests/operator.rs:83` imports `spl_token_interface::state::Mint` (the legacy
interface) and `:559` allocates `vec![0; SplMint::LEN]` = 82. Each side of the
seam tests against its own fabrication, and the two fabrications are mutually
incompatible.

**Probe:** make `mint_data()` at `tests/operator.rs:558` return the 238-byte
shape the program test already builds, then
`cargo test -p dclutch-rational-representation-v2-operator --test operator`.
Pure Rust, no SBF build.

**RESOLVED 2026-08-29 (`d9018470`) — see
`docs/evidence/TOKEN_2022_MINT_EXTENSION_2026_08_29.md`.** The probe above was
run and taken further: the fixture is now built by the official Token-2022
library rather than restated by hand. `authenticate_mint` reads through
`Token2022BehaviorProfileV2::read_mint`, a new entry point that authenticates
everything `check_mint` does except the supply, which this caller legitimately
discovers and stages for the program to pin. The control is two-sided — the real
shape builds all five actions, the truncated 82-byte shape is refused by every
builder — and the sweep found this was the tree's only wrong `Mint::parse`
caller; every other one is a collateral Mint or already slices to `MINT_BYTES`.

---

<a name="5"></a>
## 5. Canonical Token-2022 ATAs are 170 bytes; the account gate is exactly 165 — always-refuses

Both builders derive the actor's shard and receipt accounts as canonical
associated token addresses seeded with the Token-2022 program id —
`crates/dclutch-rational-representation-v2-operator/src/lib.rs:1017-1021`,
`:1127-1131` (pinned with `observed.key != expected_key` at `:1169`), and
`packages/dclutch-sdk/lib/rationalOpenChainV4.ts:139-141` with the middle seed
hardcoded. Those addresses are then read through an exact-165 gate
(`crates/dclutch-token-svm/src/behavior_profile_v2.rs:260`; TS at
`packages/dclutch-sdk/lib/rationalTokenV2.ts:263`, applied at
`rationalOpenChainV4.ts:364,375,376`).

An ATA address is a PDA of the Associated Token Account program, so only that
program can create an account there — and for Token-2022 it creates
165 + 1 type byte + 4 TLV header = **170 bytes** (`ImmutableOwner`, from
`spl-associated-token-account-2.3.0/src/processor.rs:121-124,139`).
170 ≠ 165, for the whole asset class.

Nothing in the repo exercises the ATA program: every test *plants* accounts at
derived ATA addresses via `add_account`, and even the real-ELF
`crates/dclutch-token-svm/program-test/tests/behavior_v2.rs` creates accounts
from a fresh keypair, never an ATA.

The rest of the protocol shows the intended pattern and does not have this
problem: `direct_token_setup_v1.rs:659-745` creates its participant token account
as a **dClutch PDA**, not an ATA, at exactly 165. The representation route is the
outlier — and the on-chain program imposes no ATA constraint at all
(`rational_representation_v2.rs:1012` accepts whatever key the request names), so
the ATA policy exists only in the builders that cannot satisfy their own reader.

**Probe:** `Token2022BehaviorProfileV2::check_account` against a 170-byte
`ImmutableOwner` account — `cargo test -p dclutch-token-svm`, no SBF build.

---

<a name="6"></a>
## 6. The gauntlet journey still asserts the poststate `da5460b3` removed — always-refuses

`da5460b3` made Core's `ExecuteProvider` a live route that deliberately **stops**
before the terminal transition: it asserts the Market is byte-unchanged
(`programs/dclutch-core-sbf/src/execute_provider_v3.rs:139`) and returns `Ok(())`
(`:142`), with `persist_state` removed from its imports (`:43`). The terminal
transition moved to a separate `Action::AdmitTerminal`.

`/Users/ember/dev/dclutch/tools/gauntlet/journey/src/provider.rs:572` still
demands the old poststate:

```rust
if terminal.phase != Phase::Terminal {
    return Err(Error::new(format!(
        "the provider execution left the Market at {:?}, not Terminal", terminal.phase)));
}
```

Every successful `ExecuteProvider` now leaves `phase == Open`, and the journey
never issues `AdmitTerminal` — `build_resolution_admit_terminal_v3` appears
nowhere under `tools/gauntlet/journey/src/`. No input satisfies both sides. The
run records the stage refused, pushes an `unexpected_refusals` entry, and
`resolution::retire` reports `"blocked"`
(`tools/gauntlet/journey/src/resolution.rs:824`).

The evidence contract encodes the removed transition too:
`tools/gauntlet/journey/bindings.json` binding[17] still declares *"Poststate
asserted: Phase Terminal…"*, and the stage label at `provider.rs:557` is now
false.

This matters more than an ordinary stale harness: it is the check that
`docs/guides/devnet-pyth-market-open.md` (`35af6dc8`, written hours earlier)
points operators at for an offline lifecycle check.

The two consumers that *were* moved show the correct shape:
`crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs:2222-2231`
asserts `Phase::Open` and then admits; and
`tools/local-validator/bootstrap/successor/src/flagship_resolution.rs:806-826`
classifies the new tuple correctly.

---

<a name="7"></a>
## 7. `claimsCustodyReplayV1.md` drops the one coordinate a builder cannot reconstruct — always-refuses

`docs/reference/abi/claimsCustodyReplayV1.md:42-56` states
`CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1 = 15` and then enumerates **14**
coordinates: `RENT_SYSVAR_V1 | 11` (`:54`) jumps straight to
`CUSTODY_PROGRAM_V1 | 13` (`:55`). `REPLAY_ACCOUNT_RENT_REFUND_V1 = 12` is absent
from the whole file.

The program requires it twice —
`programs/dclutch-claims-sbf/src/custody_replay_v1.rs:143` (`RENT_REFUND = 12`)
and `:389-390` (`rent_refund.key.to_bytes() != request.rent_refund ||
!rent_refund.is_writable`) — and Custody re-demands it at
`programs/dclutch-custody-sbf/src/lib.rs:662,666`.

**Why the omission is fatal rather than cosmetic.** An implementer reading only
the ABI produces either a 14-account frame (refused at `custody_replay_v1.rs:230`)
or a 15-account frame with a guessed coordinate 12 — but coordinate 12 must equal
`CoreState.rent_beneficiary` (`:328`), the *only* coordinate in the frame that is
neither a well-known program/sysvar nor derivable from the wire. It must be read
out of the Core Market account. There is no 15-account frame the doc's
information is sufficient to build.

**Cause, exactly.** `apps/dclutch-web/lib/generated/claimsCustodyReplayV1.ts:29`
gained that export in `e53efbe0`; `docs/reference/` was last regenerated in
`0617380b`, which is older:

```
$ git diff --stat 0617380b..HEAD -- apps/dclutch-web/lib/generated/
 claimsCustodyReplayV1.ts | 1 +
 refusalRegistryV1.ts     | 2 +
 routeCensus.ts           | 74 +++++++++---------
$ git diff --stat 0617380b..HEAD -- docs/reference/
 (empty)
```

`tools/genref/generate.mjs:286-289` emits every `export const NAME = <int>`, so a
regenerate restores it. **The gate that would have caught this is wired
nowhere:** `tools/genref/generate.sh --check` has exactly one caller, the
manually-invoked `tools/release/final-generated-convergence.py:220-224` — no
workflow, no hook, no gauntlet stage. `docs/evidence/ASPIRATION_LEDGER_2026_08_27.md:1886` already
knows. Two further generated surfaces are stale by the same commit
(`refusalRegistryV1`, `routeCensus`, the latter feeding `docs/reference/routes.md`).

The **code** is right: `packages/dclutch-sdk/lib/claimsCustodyReplay.test.ts:241-246`
pins `frame[REPLAY_ACCOUNT_RENT_REFUND_V1]`.

---

<a name="8"></a>
## 8. Two Structured V2 seed orders no adapter could execute — fixed in `fb076ec6`

A repo-wide sweep of the 32-byte PDA seed maximum — all 1,174
`find_program_address` / `create_program_address` sites (894 literal seed arrays
plus 709 typed-seed-struct `as_slices()`), every named domain resolved to its
literal and measured — found **the live on-chain seed surface clean** and exactly
two domains over the line, in exactly the one seed-defining crate with no guard:

```
34  STRUCTURED_RECEIPT_MINT_PDA_SEED_V2  = b"dclutch:structured-receipt-mint:v2"
35  STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2 = b"dclutch:structured-shard-custody:v2"
    — crates/dclutch-structured-v2-contract/src/seeds.rs:83, :86
```

`find_program_address` refuses every bump for a seed over 32 bytes, so neither
resource had a derivable address.

**Verdict, stated precisely rather than inflated: not a live outage.** Decision
0011 §3b routes Structured to the chain through the Rational child ABI, and the
constants are referenced nowhere outside their own crate — no program, no TS, no
ABI doc, no fixture. What is real is that the module's stated job is to be *"a
host-side authority on the exact seed ORDER"*, and two of the three orders it
published were orders no adapter could ever execute. Anything built on them would
have been born dead.

**Fixed** in `fb076ec6`: both domains abbreviated following the precedent this
codebase already set for `dclutch:projected-custody-caller:v1`
(`crates/dclutch-custody-contract/src/projected.rs:44-58`), plus the compile-time
guard the crate lacked. Migration cost zero — no account exists at the old
addresses, and none could.

The existing seed-order tests compared `as_slices()` against the very constant
they were built from, so they held for any spelling and were blind to the bytes;
two tests were added that are not (the literal spellings, and the length bound).

**Probe pair, both run.** Failing: the pre-fix 34-byte spelling under the new
guard is a `rustc` `E0080` compile error — the guard bites. Passing: the crate's
seed tests 10/10, and `cargo check -p dclutch-structured-v2-contract -p
dclutch-structured-v2-operator --all-targets` clean.

**The guard gap this exposed.** 78 seed-domain constants exist; 27 carry the
compile-time assert, 51 do not. Fully guarded today: `dclutch-dealer-codec`,
`dclutch-resolution-codec`, `dclutch-record-contract`,
`dclutch-release-set-contract`, `dclutch-source-contract`,
`dclutch-capability-program-contract`, `dclutch-capability-seal-contract`,
`programs/dclutch-trading-sbf`. Unguarded and worth the one-line addition, in
rough order of exposure: `dclutch-capability-contract` (8 domains — and
`SVM_MAX_PDA_SEED_BYTES = 32` already sits unused at `src/lib.rs:51`),
`dclutch-claims-svm` (8), `dclutch-rational-representation-v2-contract` (5),
`dclutch-custody-contract` (2 of 4), `dclutch-direct-codec` (1 of 3),
`dclutch-rent-contract` (2), `programs/dclutch-dealer-sbf` (3), and
`programs/dclutch-general-accelerator-sbf`'s test caller at **31 bytes** — one
character from unbuildable.

**Class 2 (same address, two derivations) came back clean.** 44 domains are
derived at more than one site and 28 literals are shared between the Rust tree
and the TypeScript clients; every one agrees on arity, order and segment
semantics. Apparent arity differences are all the `create_program_address` bump
convention. Today's dealer-batch fix is confirmed landed, with its regression
guard in place: the lone 3-arity site is the deliberate `assert_ne!` at
`crates/dclutch-operator/src/dealer_scenario_checkpoint_v1.rs:2770`, pinning that
the three-seed spelling is *not* the address Custody signs. The one genuine
class-2 defect found anywhere is [#3](#3), and it was found by reading the two
sides rather than by the arity sweep, because both spellings are literal arrays
of different shapes in different files.

---

<a name="12"></a>
## 12. `ActivateCapability` requires seven repeated keys that its own census forbids — always-refuses

This is defect class 3 exactly: one side *requires* a repeated key while the
other side's no-duplicate census *forbids* it.

`programs/dclutch-core-sbf/src/capability.rs:81-85`:

```rust
if request.action == Action::CloseCapability {
    require_close_capability_aliases(accounts, funding_header.physical_count())?;
} else {
    require_distinct(accounts)?;
}
```

`require_distinct` (`programs/dclutch-core-sbf/src/frame.rs:550-559`) refuses if
any two accounts in the whole top-level vector share a key.

**But the route structurally requires seven repeats.** Core forwards
`route.child_tail` verbatim to Trading behind a `4+F`-long prefix
(`capability.rs:848-866`), and Trading's `AuthenticatedSuffixV2::parse`
(`programs/dclutch-trading-sbf/src/outer.rs:867-882`) re-reads cache, Core
program/programdata, Trading program/programdata, Registry, rent and system at
child-tail coordinates 8…15 — the same accounts Core already carries in its own
fixed frame.

**Minimal argument, needing no state reasoning at all:**

1. Core requires `accounts[14+F]` to be the real Rent sysvar —
   `Rent::from_account_info(route.rent)` at `capability.rs:95`, where
   `route.rent = accounts[14+F]` (`capability.rs:312`).
2. Trading requires `family_accounts[14] = accounts[30+F]` to be the real Rent
   sysvar — `outer.rs:880`.
3. `14+F ≠ 30+F`, and there is exactly one Rent sysvar key.
4. `require_distinct` refuses with `CoreSbfError::AccountFrame` before
   `Route::parse` even runs.

The same argument holds independently for the Registry program (`13+F` vs
`29+F`), the Trading program (`9+F` vs `27+F`), the Core program, the activation
cache and both programdata accounts — **the seven pairs
`close_alias_pairs()` already admits**
(`crates/dclutch-market-core-codec/src/capability.rs:142-152`), whose own doc at
`capability.rs:245-251` states the requirement is Trading's and not
close-specific: *"Trading deliberately authenticates those same accounts again at
child-tail coordinates 8..14. They therefore appear twice in the top-level
frame."* `AuthenticatedSuffixV2::parse` is called from **both**
`process_activation` (`outer.rs:248`) and `process_close` (`outer.rs:573`).

**Provenance.** `480e18f0` ("core: admit exact native-close aliases",
2026-08-28) replaced an unconditional `require_distinct` with the two-arm form.
It fixed close and left activate on the old, now-unsatisfiable, check.

**Why no test caught it (class 5).** The only program test is
`programs/dclutch-core-sbf/tests/capability_close_alias_program_test.rs`, which
is close-only. The Trading side is exercised by
`programs/dclutch-trading-sbf/program-test/tests/activation.rs:1288-1310`, which
builds the child instruction directly against a Core **stub**
(`program-test/test-programs/core-caller/src/lib.rs:48-60`) that forwards its
accounts verbatim and runs no distinctness check and no Core fixed frame — the
stub is strictly more permissive than real Core in exactly the dimension that
breaks.

The repo half-knew: `docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md:2084-2086`
records the route as "undriven", and `docs/ledger/board-archive-2026-08-27.md:8358-8360`
warns that `require_distinct` is global — without noticing the seven existing
aliases already collide. **The route is not undriven for want of a builder; it
cannot be driven.**

**Fixed in `3b98ea3a`,** in the shape HARDEN established on Custody this morning
— never a blanket relaxation. Both arms now run the alias-aware census, which
pins each of the seven **positively** first (required to be the specific account
the frame says it is) and only then excuses that exact pair from the all-pairs
duplicate check. A third copy of an aliased account, a cross-pair swap, a shifted
pair and every other collision still refuse, and the pre-existing close test
already covers each. The function is renamed for what it checks rather than for
the one action that used to call it.

**The probe, and it is the finding stated as an assertion:** the new test says
the canonical frame is exactly what the alias census demands **and** exactly what
`require_distinct` refuses — two halves of one frame. That second assertion is
what would have failed before this commit, and it is why the route was not
undriven for want of a builder. `cargo test -p dclutch-core-sbf --lib` → 24
passed, 0 failed.

---

<a name="13"></a>
## 13. `5ca145e8` made the funding source the fee payer; three consumers still pin it by identity — always-refuses ×3

The index shift itself is **complete and correct**: every site that indexes the
48-account DCLTCFQ1 list by number moved with it, and the Resolution callee's
prefix→found alias census now passes where it previously could not — that was the
always-refuses this commit *fixed*. The residue is **identity drift**, not index
drift: `funding_source` stopped being the projection witness and became the
campaign's **fee payer**, and three consumers that pin it by identity did not
follow.

**13a — the DCLTPCA1 complete-key census was never re-derived (33 → 32).**
`PROJECTED_CUSTODY_ABORT_COMPLETE_KEYS_V1 = 33`
(`tools/local-validator/bootstrap/successor/src/market.rs:6582`, enforced at
`:7064`). `projected_bootstrap_compiled_geometry_v2` (`:4268-4283`) excludes
`key == payer` from the lookup table. Before the commit the funding source was a
key appearing nowhere else → 30 ALT addresses, 4 static, 29 loaded, **33**. After,
that key *is* the payer → 29, 28, **32**. The same −1 that `bba217c5` correctly
applied to the cleanup frame (`:6584-6586`) was never applied here. Every
local/devnet expiry recovery dies at `"DCLTPCA1 census refused: base 32…"` before
signing. The fixture `staged_abort_census_fixture_v1` (`:11206-11237`) builds 31
distinct keys and **never places `payer` in the frame** — it models the pre-commit
world, so its assertion at `:11606` is vacuous.

**13b — the fee payer cannot sit in a frame that forbids every signer.**
`authenticate_expired_checkpoint_v1`
(`programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs:607-632`)
refuses if `value.is_signer` for any account. But `is_signer` is a
**transaction-level** property: the fee payer is message key 0 and every
instruction naming it sees `is_signer == true` regardless of the `AccountMeta`
flag. Since `5ca145e8` the funding source *is* the campaign payer, and the builder
places it at `FUNDING_ABORT_FUNDING_SOURCE = 7` (`market.rs:7505`) in transactions
that payer signs (`:7203-7210`, `:7241-7248`). This gates all three abort routes —
DCLTCF1A (`:927`), DCLTCF2A (`:458-461`), DCLTPCA1 (`:1389-1398`). **An expired
founding can never be unwound:** principal stays in the Custody source vault and
rent stays in two ledgers plus the checkpoint, permanently.

Class 5 again, and unusually clean:
`crates/dclutch-svm-harness/tests/controller_funding_split_abort.rs:529` invents
`let funding_source = Pubkey::new_from_array([0x62; 32])` and seeds it with
`add_account`, while every transaction is paid by `context.payer` (`:787-791`).
The invented value is more plausible than the real one, so the no-signer census is
never exercised against the shape production actually builds.

**13c — lamport conservation on the funding source.** `market.rs:7676-7689`
requires `funding_source_after == before + controller_native_refund` exactly, with
`before` snapshotted at `:7133`. Between snapshot and check the payer pays for
DCLTPCA1, three routing-table publications, DCLTCF1A and DCLTCF2A. With the
funding source now the fee payer, the equality can never hold.

**13d — latent, and the reason this matters beyond the abort path.**
`5ca145e8` added the payer to `prepare_accounts` (`market.rs:5950-5956`), which is
passed as **both** `prestate_addresses` and `completion_addresses` (`:6058-6059`).
`send_durable_founding_v1` re-checks the prestate digest before signing, before
dispatch and before every resend (`:612-616`, `:651-657`, `:690-697`), so the
payer's *lamports* are now part of that digest. On a fresh single-process run
nothing spends the payer in between; on a **crash-resume** any intervening fee
permanently deadlocks the journal at *"Dispatching recovery found changed
prestate; poll the exact signature and do not resend"* — defeating the point of
the durable journal. Every other founding operation keeps mutable-balance
accounts out of these sets.

**Probes, both cheap.** 13a: set
`accounts[PREFIX + 7] = AccountMeta::new(payer, false)` in
`staged_abort_census_fixture_v1` and run
`cargo test -p dclutch-local-successor-bootstrap -- staged_abort_compiler_census`
(seconds, no validator). 13b: set the fixture's `funding_source` to the
ProgramTest payer's pubkey and run
`cargo nextest run -p dclutch-svm-harness -E 'test(real_custody_source_abort_then_controller_suffix_is_exact_and_resumable)'`
— one ProgramTest, not the suite.

Also worth naming: the DCLTCF1A pre-expiry negative probe
(`market.rs:6118-6140`) now refuses on 13b's signer census rather than on expiry,
and asserts only `fee_only_balance_change == Some(true)` — **green for the wrong
reason**, no longer testing the boundary it names. And no in-tree SVM harness
exercises the DCLTCFQ1 prepare frame at all; its on-chain behaviour is proven
solely by live successor runs.

---

<a name="latent-hazards"></a>
## Latent hazards

<a name="9"></a>
**9. Claims pins nothing about the Custody program on the replay leg.**
`programs/dclutch-claims-sbf/src/custody_replay_v1.rs:382-383` accepts any account
at coordinate 13 that is `executable` and `!= program_id`. Claims then derives the
replay PDA under it (`:410`), CPIs it with the Claims-role caller authority signed
(`:444-448`), and accepts its return data as a `CustodyReceiptV1` (`:482-486`) —
checking `replay.owner == custody_program.key` (`:490`) against that same unpinned
account. A substituted executable program satisfies all nine post-conditions by
writing them. The module docstring's stated invariant — *"two independent authors
have to agree before an account is created"* (`:404-405`) — does not hold for that
input.

Blast radius is limited: the fake replay lives at an address only derivable under
the fake program, and the real payout leg re-derives under
`frame.custody_program.key` and hits the real Custody, which self-authenticates
(`custody-sbf/lib.rs:513-521`). But the route emits a receipt whose `producer` is
the attacker's program while the module header advertises `producer` as
trustworthy (`:66-72`). The payout leg does it right —
`terminal_settlement_v3.rs:551` pins
`accounts[CUSTODY_PROGRAM].key.to_bytes() == input.custody_program`.
Cheapest closure: join coordinate 13 to the activation cache Claims has already
decoded at `:285-307`, one more `.role()` call.

<a name="10"></a>
**10. The payout leg's Custody frame has no compile-time tie to Custody's width.**
`programs/dclutch-claims-sbf/src/rational_terminal_v3.rs:451-470` builds the
14-account Transfer frame as a bare `Vec::from([...])` literal.
`dclutch_custody_contract::TRANSFER_ACCOUNT_COUNT_V1` (= 14,
`frame_spec_v1.rs:16`) is referenced **nowhere** in `programs/dclutch-claims-sbf/`.
The replay leg has exactly this guard (`custody_replay_v1.rs:153-156`) and the
payout leg does not: if Custody's Transfer frame widened, Claims would still
compile, every unit test would stay green, and the failure would surface only in
a real CPI. The 14 metas are correct today, checked coordinate by coordinate
against `frame_spec_v1.rs:223-229`. Fix is a one-line `const _: () = assert!`.

**11. Assorted, each with a `file:line` in the lane reports:**

- **`b"dclutch/direct-replay/v3"` is derived only by TypeScript** —
  `apps/dclutch-web/lib/directTransaction.ts:21,133,136` and the SDK twin. A
  repo-wide grep finds no Rust constant, no program, no test. Its sibling
  `dclutch/direct-registered/v1` at least has a Rust cross-check
  (`formal/qedsvm-direct-v12/dclutch_direct_mollusk_trace.rs:17`).
- **`b"dclutch:rational-receipt:v2"` is never derived by the on-chain program** —
  only by a test and by `apps/dclutch-web/lib/rationalRetireReceiptV4.ts:752`.
  Claims only *compares* against the persisted descriptor field
  (`rational_representation_v2.rs:596`). Deliberate per the seed contract's doc,
  but it makes the client the sole enforcer of that derivation policy.
- **`routeCensus` publishes `len ==` where the dispatcher demands `len >`** —
  `apps/dclutch-web/lib/generated/routeCensus.ts:350` vs
  `programs/dclutch-core-sbf/src/lib.rs:331`. Exactly the one length the route
  refuses. Model-level: `Selector::Length`
  (`tools/gauntlet/census/src/model.rs:89-93`) carries no operator and both
  renderers hardcode `==`. Browser label only, not the chain.
- **Direct's realm token program is mutually unsatisfiable across halves** —
  `direct_token_setup_v1.rs:500,513` hard-require Token-2022 on both the realm and
  the collateral adapter profile, while the `direct-hot` ProgramTest realm is
  legacy (`program-test/direct-hot/src/fixture.rs:1013`) and the TS registered
  builder pins legacy exclusively (`apps/dclutch-web/lib/registeredDirect.ts:553`)
  against an SDK that pins Token-2022
  (`packages/dclutch-sdk/lib/generated/directParticipantV1.ts:43`). Each harness
  exercises only its own half.
- **`founding_v5` accepts a hoard owned by either token program** without
  cross-checking the Realm (`programs/dclutch-claims-sbf/src/founding_v5.rs:677`),
  where its Core sibling does compare
  (`programs/dclutch-core-sbf/src/generic_founding_v1.rs:1161-1165`). Mitigated
  downstream by custody's pin.
- **`walletTerminalPayoutV3` admits both token programs then decodes only one** —
  `packages/dclutch-sdk/lib/walletTerminalPayoutV3.ts:537-539` vs the exact-165
  gate at `:477`. The Token-2022 branch is never exercised by its tests.
- **Coordinate 23 changes identity with the payout sign** and the ABI page does
  not say so — `terminal_settlement_v3.rs:690` requires the executable Claims
  program on the zero-payout branch while `rational_terminal_v3.rs:564-565`
  requires the caller-authority PDA on the positive branch. Intentional
  (`crates/dclutch-operator/src/wallet_terminal_payout_v3.rs:196`) and asserted
  both ways in its own test, but absent from
  `docs/reference/abi/walletTerminalPayoutV3.md`, and the fractional operator's
  profile pins that coordinate `executable: false`
  (`crates/dclutch-fractional-claim-operator/src/selected_release_v4.rs:632-638`)
  — the same fact written down twice with opposite answers.
- **`ProviderCallerV3::Resolution` is now unreachable** after `da5460b3` — dead
  but decodable, with its whole support apparatus intact
  (`crates/dclutch-resolution-codec/src/provider_v3.rs:97`;
  `provider_instruction_v3.rs:120-125,645,650,653-656,731`). Fail-closed today.
- **`dclutch:dealer-reserve:v1` agrees only by runtime equality checks**, not by
  construction: seed 2 is written three different ways
  (`dealer_reservation_v1.rs:1487,1627`,
  `dealer_scenario_checkpoint_v1.rs:817`) and pinned equal by explicit guards at
  `dealer_reservation_v1.rs:260,447,1152`. Correct today; structurally fragile
  where `CustodyVaultSeedsV1::as_slices()` cannot drift.
- **The V1 and V2 registry-continuation seams mint the same admission PDA** —
  `programs/dclutch-registry-sbf/src/continuation_v1.rs:24-172` derives the
  identical seven-seed admission and CPIs Trading with a real signer; only
  Trading's `observed.data() != instruction_data` check at `hot_v3.rs:1697`
  separates them, because the seed tuple binds the **child** bytes and the
  top-level bytes are bound in V2 alone. Marked UNRESOLVED in the test's own doc
  (`program-test/tests/registry_hot_continuation.rs:348-368`).
- **Core stamps a reauthentication it never performed** —
  `programs/dclutch-core-sbf/src/release.rs:194-308`
  (`authenticate_continuation_roles`) checks only the two account *keys* against
  the cache: no Loader ownership, no view parse, no deployment slot, no ELF
  digest, and it can never return `ReleaseSuperseded`. Yet
  `RoleBatchAdmissions::admission` (`:62-78`) emits
  `current_deployment_reauthenticated: true` unconditionally, on that path and on
  the fully-checking one alike. Sound only because `batch_request_digest` is an
  admission seed — an argument stated nowhere in the file.
- **Accelerator registry-mode requires a writable Market where the ordinary path
  merely permits one** — `hot_v3.rs:1288-1291` is a demand, `hot_v3.rs:10542` a
  permission, and builders copy `hot.accounts` verbatim
  (`program-test/direct-hot/src/waist.rs:874`). Two spellings of "market union"
  that are not the same predicate.
- **The slot pin is implemented three times** —
  `crates/dclutch-registry-activation-auth-v1/src/lib.rs:377-440`,
  `crates/dclutch-shadow-accelerator-auth-v4/src/deployment.rs:59-134`, and
  `programs/dclutch-core-sbf/src/infrastructure.rs:369-449`. Equivalent today,
  sharing only `slot_pinned_release_elf_digest_v1`; the auth-v1 copy omits the
  `program.key == programdata.key` check its sibling has at `deployment.rs:71`
  (covered on live routes, not by the function).
- **The registry-continuation prefix count is declared six times** — all
  consistent at `1 + 2·roles + 1`, but the only builder any live test uses is an
  inline `vec!` literal with no constant at all
  (`program-test/direct-hot/src/waist.rs:874-881`), while the operator crate's
  `build_registry_hot_continuation_v2` has zero callers and zero tests.
- **Test binaries are not what the witnesses claim.** No test loads a classic
  `spl_token.so`; `solana-program-test` genesis installs p-token 1.0.0 at Tokenkeg
  and Token-2022 **v10** at TokenzQd, so
  `tools/gauntlet/claims-custody/custody-witnesses.json:14-16` proves the address
  was invoked, not the mainnet binary. Nothing in the repo names
  `solana-program-binaries`, so a harness bump silently changes the token binary
  behind ~14 SBF tests. Separately
  `program-test/fractional-atomic/tests/fractional_atomic.rs:107-121` claims a
  pinned v11 ELF and asserts no digest, where its two siblings do.

<a name="benign-verified"></a>
## Benign, verified rather than assumed

- **Core↔Resolution after `da5460b3`.** Caller-PDA seeds
  `[b"dclutch:role-authority:v1", release_set, market, role, source_state,
  parent_request_digest]` match segment for segment at all six derivation sites:
  `execute_provider_v3.rs:188,230`, `provider_instruction_v3.rs:660`,
  `provider-transport-v3-operator/src/lib.rs:640-649,1064-1080`,
  `provider_finalized_projection_v3.rs:500-527`,
  `resolution_composition_v3.rs:419-428`. No old-seed survivor anywhere, and no
  TS/ABI mirror of this seam exists to drift. Account frames, writable and
  executable sets and the no-duplicate census agree exactly. The index-0 signer
  asymmetry (Core demands not-signer at top level, Resolution demands signer) is
  intentional and reconciled by Core's CPI promotion at `:216` — it is what kills
  the direct top-level route, asserted at
  `resolution_core_v3_lifecycle.rs:2166-2181`.
- **Market-open boundary.** `35af6dc8` encodes no program bound; the single
  on-chain bound lives in Resolution
  (`crates/dclutch-source-contract/src/provider_join_v2.rs:230-246`) and Core
  encodes none, so there is nothing to disagree. The journey's mirror
  (`tools/gauntlet/journey/src/provider.rs:676-740`) reproduces it faithfully.
- **Pyth fixtures are captured devnet reality, not invented** — 134-byte price
  update with the real Anchor discriminator, real receiver/router ELFs at the real
  id (`crates/dclutch-svm-harness/tests/support/pyth_provider.rs:98`). Shape
  assertions there bite.
- **Claims↔Custody replay/payout prestate pair is satisfiable.** The class-(c)
  hazard specifically looked for is absent: the payout leg demands
  `Phase::Terminal` (`terminal_settlement_v3.rs:597`) and the replay-creation leg
  imposes no Core phase requirement at all, so a replay can legitimately be
  created after resolution. The revision chain closes end to end, and
  `CustodyVaultSeedsV1` deliberately excludes the role while `CustodyReplaySeedsV1`
  includes it — the decision-0008 fix for the previously-dead cross-role pair.
- **L3 retirement handoff** agrees on every axis: identical 23-account
  all-pairs censuses, identical writable/executable masks, both demanding
  `Phase::Retiring`, identical Trading→Core replay seeds, and the digest
  asymmetry (`hash(request.to_bytes())` vs `hash(request_bytes)`) is provably
  equal because `decode` is strictly canonical.
- **Checked-variant discipline is clean.** `crates/dclutch-token-svm/src/instruction.rs`
  builds only `TransferChecked`, `InitializeAccount3`, `CloseAccount`, `Revoke` —
  no unchecked `Transfer` exists anywhere in the tree. Decimals agree across
  writer, reader and profile.
- **A pin-vs-census contradiction exists but is doubly dead** —
  `signed_delta_v3.rs:576-577` requires `caller_program == claims_program` for
  `CallerRole::Claims`, which `claims_composition_v3.rs:152-158` forbids by
  no-duplicate census; both sides refuse Claims-role plans on that path anyway.
- **`marketDiscovery.ts:589-591` is the one decoder that gets dual-program
  right** — owner compared against the Realm-decoded program, width checked as a
  minimum. It is the pattern the other four should follow.

---

## What this sweep says about the method

Nearly every always-refuses finding shares one shape: **each side was tested
against a fixture that same side authored.** The Token-2022 writer and its
readers each fabricate the mint bytes they expect (#2); the operator builds legacy
mints while the program requires Token-2022 ones (#4); the ATA builders plant
accounts rather than creating them (#5); Trading's record test derived with the
checker's own spelling (#3); the Structured seed tests compared a constant to
itself (#8); the capability activation test runs against a Core *stub* that is
strictly more permissive than Core in exactly the dimension that breaks (#12);
the abort fixture invents a `funding_source` of `[0x62; 32]` where production uses
the fee payer, which is the whole defect (#13b). In every case both sides were
green and the composition was dead.

The corollary is the sharper half: **a green suite is evidence about fixtures,
not about seams.** Four of these routes had never executed at all — the
admitted-AOT lane (#3), `ActivateCapability` (#12), the abort recovery path
(#13b), and the Structured seed orders (#8) — and nothing in the test suite was
capable of saying so. Where a route *had* moved recently, the drift landed in the
place the commit did not look: `480e18f0` fixed one arm of an `if` and left the
other (#12); `5ca145e8` shifted every numeric index correctly and changed an
*identity* three consumers still pinned (#13); `da5460b3` moved a poststate and
left the harness that asserts it (#6).

The two guards that actually caught things are cheap and mechanical: a
compile-time seed-length assert, and a `const _: () = assert!` tying one
program's frame width to the other's declared constant. Where they exist they
work — the Claims↔Custody replay leg has the second one and is clean; the payout
leg lacks it and is the hazard. The recommendation is to finish that coverage —
51 unguarded seed domains, the payout leg's missing width tie, and wiring
`tools/genref/generate.sh --check` to anything at all — rather than to write more
tests that each side can pass alone.

The cheapest structural lesson is #3's: the ten calls that split one identity in
half became five that take the pair, so the defect is no longer *expressible*
rather than merely fixed. Where a seam has one truth, one side should own its
spelling and the other should consume it.
