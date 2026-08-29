# The lifecycle writes a Mint its own terminal path can burn, 2026-08-29

SEAM found it and would not fix it: `rational_lifecycle_v2::initialize_closeable_mint`
created the rational receipt and shard Mints with **only** `MintCloseAuthority`,
at 202 bytes, while three on-chain sites burn those exact PDAs through the
`PermissionedBurn` extension. Token-2022 extensions are init-time-only, so every
Mint that route founded was unburnable **permanently** and no upgrade could
repair one. This records the fix and the executed proof.

Commits: `f7c960b9` (writer, reader, shared TLV walker, campaign join),
`bb625688` (the hostile that had been describing us). **Both change the Claims
ELF — cohort-critical.** Devnet untouched.

## What was actually wrong

Not the readers. The question "which side is right" was settled by what the
terminal path needs, and the protocol really does burn through the extension:
`rational_representation_v2.rs:1364` (`BurnReceipt`/`BurnShard`) and
`fractional_atomic_v3.rs:958`/`:1003` (`WholeUnwrap`) all emit
`permissioned_burn` burn instructions against these Mints, and the committed
behaviour preimage names `mint-required=MintCloseAuthority+PermissionedBurn`.
The writer built a Mint its own terminal path could not burn.

The repository already contained the executable proof, filed as reassurance.
`rational_representation_v2_program_test.rs`'s
`a_receipt_mint_missing_its_burn_role_refuses_at_the_first_issue` builds
exactly 202 bytes with the close role only, asserts the refusal, and its comment
called that an under-configured *hypothetical* founding. It was not
hypothetical. It was, byte for byte, what the shipped writer produced.

**Why it survived a green tree.** The two campaigns are disjoint. The lifecycle
campaign activates and retires against the real Token-2022 v11 ELF and never
issues; the representation campaign issues, but installs its Mints as
hand-planted `add_account` bytes carrying a fabricated `PermissionedBurn` TLV —
a shape no dClutch code path produced. Each side's fixture invented exactly the
bytes that side expected, so both were green and the composition was dead.

## The fix

**Writer.** `initialize_closeable_mint` issues `permissioned_burn::initialize`
between the close-authority instruction and `InitializeMint2`, with the
representation authority in every role, and allocates the resulting 238 bytes.
Ordering matters and is not cosmetic: Token-2022 admits extension initialization
only before `InitializeMint2`.

**Reader.** `closeable_mint::check_mint` was a fixed-offset reader for one TLV
at 166/168/170 behind an exact-202 length gate. It now walks the real TLV
storage and requires both extensions, in either relative order, because
Token-2022 pins that order nowhere. The distinction is the point: a close-only
Mint is now refused **because `PermissionedBurn` is absent**, which is a fact
about the Mint, where a length check is only a fact about its size. Every
authority role is a named parameter rather than an assumed alias of the one
beside it, so a route that later separates them cannot pass by accident.

**The walker moved to one shared module** (`token-svm/src/tlv.rs`). Two profiles
reading the same account bytes through two parsers of different rigour is *how
this survived*: `behavior_profile_v2` already walked properly and demanded both
TLVs; `closeable_mint` did not. Neither can drift again.

**Rent.** 202 → 238 moves the rent-exempt principal from 2,296,800 to 2,547,360
lamports per Mint. `receipt_rent_principal` and `shard_rent_principal` are wire
fields compared for **exact** equality, so this is a wire-visible economic
change — but a **pre-release** one, not a migration. No live market uses the
rational/structured representation family and nothing under `tools/` drives this
route, so no permanently broken Mint exists yet. The debt would have begun
accruing the first time a campaign founded one for real.

The coupling turned out smaller than sized, for a reason worth keeping: **the
tree pins no rent lamport figure for this account anywhere.** All three sites
recompute `Rent::minimum_balance(TOKEN_2022_CLOSEABLE_MINT_BYTES_V2)`, and the
TypeScript retire path reads the live account's own length
(`rationalRetireReceiptV4.ts:803`). There is no activation-side rent builder in
TypeScript at all. So the principals moved with the width without a single
value being restated. A fan-out that computes is a fan-out that does not fan.

## The proof

Three campaigns, all against real ELFs, all green at `bb625688`.

| Campaign | Result |
| --- | --- |
| `program-test/rational-lifecycle` (real Token-2022 v11 ELF) | 2/2 |
| `tests/rational_representation_v2_program_test` | 21/21 |
| `program-test/fractional-atomic` (both terminals, real Custody) | 13/13 |

The lifecycle campaign now asserts **the join that had never been executed**.
`assert_lifecycle_mint_is_terminally_burnable` takes the bytes the real
Token-2022 v11 program wrote for the receipt and shard Mints in that
transaction, and hands them to `Token2022BehaviorProfileV2::check_mint` — the
reader `BurnReceipt`, `BurnShard` and `WholeUnwrap` actually gate on — as well
as to the lifecycle's own profile. It admits them. No fixture is involved on
either side of that assertion; it is one route's real output read by the other
route's real reader.

### The controls, and what they found instead

Run in an isolated worktree, never by mutating the shared tree. A green campaign
plus the author's say-so is not a verification, so the question was whether the
new assertion actually fails on the old writer or would have passed either way.

**Control A — "238 bytes, extension removed" is not a constructible state.**
The intent was to separate the width from the extension. Token-2022 v11 does not
permit the separation: `InitializeMint2` refuses with `InvalidAccountData` on a
238-byte account carrying only the one `MintCloseAuthority` TLV. The program
requires the allocated length to be exactly the length its initialized
extensions imply.

**That is a structural guarantee, and it is the most useful thing these controls
produced.** The width and the extension set are atomic: 238 bytes without
`PermissionedBurn` cannot be initialized, and `PermissionedBurn` at 202 bytes
has nowhere to live. A future edit that gets either half of this pair wrong
does not produce a subtly broken Mint — it produces no Mint at all, and the
transaction fails at the Token program. The defect that this document exists to
fix cannot silently recur in that direction.

**Control B — the old writer, exactly (two CPIs, 202 bytes) — refuses on-chain
at `0x5216` before any test looks at bytes.** That is
`RationalLifecycleSbfErrorV2::Token` raised by the program's *own* post-create
reader `authenticate_closeable_mint`, which now calls the strengthened profile.
So on the fixed program the old writer cannot commit at all; the on-chain gate
is strictly stronger than the campaign's assertion, which is why neither A nor B
ever reaches the assertion under test.

**Control C — the one that actually answers the question.** Old writer plus
`authenticate_closeable_mint` stubbed to `Ok(())`, so the old bytes commit and
the helper finally runs. It fails inside
`assert_lifecycle_mint_is_terminally_burnable` with
`closeable lifecycle Mint profile: InvalidExtensionLayout` — **after** the
`"lifecycle Mint width"` assert has passed, because 202 == 202 was true at that
moment. The refusal is a pure extension fact and not a size fact. The assertion
is load-bearing and is not a tautology.

**Cost.** The added CPI consumes 1,067 CU inside Token-2022, ~2,321 with CPI
overhead, per Mint. That figure is this change's own, read off the Token-2022
instruction log, and is the only number here that belongs to this change alone.

The committed-leg totals are not, and the difference is worth stating rather
than averaging away. Measured at `f7c960b9`: `activate_receipt` 254,337,
`activate_coordinate` 405,138, `retire_coordinate` 340,832, `retire_receipt`
264,515. Re-measured at `2e3257d6` an hour later: 237,836 / 448,637 / 343,831 /
267,514. Concurrent lanes changed shared Claims code between the two runs, so
neither set attributes to this fix — quote them only with the commit beside
them. All eight are far under the 1,400,000 meter the campaign's census witness
pins, which is the property that actually had to hold.

## Named debt, not fixed here

**SEAM §4 is the same defect seen from the wallet side, and this fix makes its
disjointness exact.** `dclutch-rational-representation-v2-operator`'s
`authenticate_mint` (`src/lib.rs:1151`) calls `Mint::parse(observed.data)`,
which refuses anything but exactly 82 bytes. Both of its call sites — `:667` and
`:1045` — are precisely the receipt Mint and the shard Mint this lifecycle
writes, which are now definitively 238 bytes. 82 and 238 are disjoint, so every
`construct_*` builder behind it refuses before it can build a transaction.

It is deliberately not fixed here, and the reason is a design question rather
than caution: `authenticate_mint` **reads** the supply rather than pinning it,
and both extension-aware profiles in `token-svm` require an expected supply.
Passing back a supply read from the same bytes would be a vacuous assertion. A
correct fix needs a third, deliberately weaker public entry point in `token-svm`
that authenticates shape, authorities, decimals and freeze while returning the
supply — a new public API, with the judgment that implies. Its probe is cheap
and pure-Rust: make `tests/operator.rs:558`'s `mint_data()` return the 238-byte
shape, then `cargo test -p dclutch-rational-representation-v2-operator --test
operator`.

## The doctrine this cost

A shape assertion that a wrong-but-plausible fixture satisfies is not evidence.
The old reader's exact-202 length gate would have been satisfied by any 202-byte
buffer with the right 36 bytes at offset 166 — including one with the wrong
extension entirely, had the type check not been separately present, and
including the exact broken output of its own writer. It refused the correct
Mint and admitted the broken one. **Parse the structure; do not measure the
building.**
