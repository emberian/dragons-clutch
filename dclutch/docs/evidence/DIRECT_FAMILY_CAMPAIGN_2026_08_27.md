# Direct family campaign — 2026-08-27

Source commit `7123164c7bcc58f636da4acac2ce7ba8c394cca2`. Every artifact below
was built from `git archive HEAD` of that revision into a scratch tree, never
from the shared working directory, which half a dozen lanes were editing
throughout. `cargo-build-sbf 4.0.0`, platform-tools v1.53, SBF rustc 1.89.0.
Host tests used solana-program-test 4.2.1 (the harness) and 4.3.0-beta.2 (the
gauntlet producer).

This is local real-SVM evidence for exact artifacts. It is not a validator
campaign, not devnet, not mainnet, and not a claim that any Solana program here
is formally verified.

## What moved

`direct-aot/process_instruction` is the first Direct row in the execution
census to read **EXECUTED**. It reads:

```
| EXECUTED (49x via direct-aot-programtest) | `direct-aot/process_instruction` |
```

and two of its four refusal codes moved from *never raised* to **OBSERVED**.
Across the whole census that is 11 → 13 executed routes and 6 → 10 observed
refusal codes, the Direct half of which is one route and two codes.

The other two Direct-adjacent bodies of evidence in this document — the
registered controller lifecycle and Direct through common Hot — moved no census
row, and the last two sections say exactly why.

## 1. The stateless Direct V2 AOT accelerator

`tools/gauntlet/direct/` builds the real `dclutch_direct_aot_sbf.so` and drives
49 labelled transactions through it under `solana-program-test` at the
canonical 1,400,000 CU limit and the default 32,768-byte BPF heap.

| artifact | bytes | SHA-256 | frame diagnostics |
|---|---:|---|---:|
| `dclutch_direct_aot_sbf.so` | 26,848 | `e5d2223ec0cc2406f9d7e2729497f472e5c589f05d00bcbbd2fdbbb4b17e6806` | 0 |

The digest reproduced byte-for-byte across two independent builds. The tier's
build stage **refuses to produce evidence at all** if the count is nonzero:
`cargo build-sbf` exits zero on stack-frame-overwrite diagnostics, and an
artifact the toolchain calls potentially-undefined has no business in a
campaign.

### The shape of a refusal on this route

The accelerator holds no state and takes no account. A **semantic** refusal is
therefore a canonical 160-byte `Refused` acknowledgement published as return
data by a transaction that **succeeds**. Only the wire and frame adapters raise
the program's own error taxonomy. The census records all 29 admission refusals
as EXECUTED, because that is what the chain reports; binding them as refusals
would credit two codes the program never raised.

### Admitted frames

The canonical frame is fill-or-kill on both sides at the full signed quantity
2,000, execution price 500,000 against a scale of 1,000,000, and a 25 bps venue
rate that policy, seller and buyer all signed: gross 1,000, floor fee 2.

| case | CU | gross | fee |
|---|---:|---:|---:|
| the canonical fill-or-kill fill | 2,650 | 1,000 | 2 |
| a partial fill under a resting lifecycle | 2,658 | 500 | 1 |
| the minimal fill whose floor fee rounds to zero | 2,658 | 1 | 0 |
| a price exactly at the signed seller limit | 2,650 | 800 | 2 |
| a price exactly at the signed buyer limit | 2,650 | 1,200 | 3 |
| a buyer holding exactly gross plus fee | 2,650 | 1,000 | 2 |
| a seller holding exactly the filled claims | 2,650 | 1,000 | 2 |
| a slot exactly at the tightest window open | 2,650 | 1,000 | 2 |
| a slot exactly at the tightest window close | 2,650 | 1,000 | 2 |
| a fee rate exactly at the denominator | 2,650 | 1,000 | 1,000 |
| an execution price exactly at the price scale | 2,650 | 2,000 | 5 |
| the last outcome coordinate in the Market | 2,650 | 1,000 | 2 |

Every gross and fee is cross-checked by a witness against
`tools/gauntlet/direct/expectations.json`, which derives all twelve by hand from
the stated quote equation and never reads the campaign output.

### Semantic refusals

Each is the near twin of an admitted frame above, so the campaign pins the
boundary rather than a distant point on the wrong side of it.

| case | CU |
|---|---:|
| a zero fill | 1,403 |
| a partial fill against a fill-or-kill intent | 1,448 |
| a fill above the signed maximum | 1,447 |
| a lifecycle that is neither FOK, IOC, nor GTC | 1,446 |
| a quote whose exact division has a remainder | 1,726 |
| a matcher price one below the seller limit | 1,470 |
| a matcher price one above the buyer limit | 1,472 |
| an execution price above the price scale | 1,475 |
| a Market that is not open | 1,401 |
| a slot one before the tightest window open | 1,410 |
| a slot one after the tightest window close | 1,408 |
| a maker generation skew | 1,431 |
| a selected-outcome mismatch | 1,435 |
| an outcome coordinate outside the Market | 1,440 |
| two makers on the same side | 1,416 |
| a seller presented as the buy side | 1,414 |
| one maker filling against itself | 1,447 |
| two intents signed for different Markets | 1,419 |
| a seller one claim short of the fill | 1,907 |
| a buyer one unit short of gross plus fee | 1,925 |
| a seller collateral balance that would overflow | 1,945 |
| a venue collateral balance that would overflow | 1,952 |
| a buyer claim balance that would overflow | 1,936 |
| a zero price scale | 1,475 |
| a venue fee rate the makers did not sign | 1,479 |
| a seller fee rate the policy did not set | 1,479 |
| a replay nonce that is not the maker successor | 1,458 |
| a saturated maker replay nonce | 1,466 |

### Physical refusals

These raise the program's taxonomy and the transaction fails.

| case | CU | refusal |
|---|---:|---|
| an account-bearing invocation | 199 | `NonStatelessFrame` (0) |
| an empty instruction | 98 | `InvalidRequest` (1) |
| a truncated request | 166 | `InvalidRequest` (1) |
| an over-long request | 166 | `InvalidRequest` (1) |
| a foreign request magic | 101 | `InvalidRequest` (1) |
| a nonzero reserved request span | 130 | `InvalidRequest` (1) |
| a request declaring a foreign scalar width | 345 | `InvalidRequest` (1) |
| a request declaring a foreign identity width | 347 | `InvalidRequest` (1) |

The CU ordering is itself a check on the decoder's order of operations: the
empty instruction dies at the length floor (98), the foreign magic at the magic
compare (101), the dirty reserved span after that (130), the width mismatch
after the header parses (166), and the two internally-consistent foreign bank
widths cost most (345/347) because they decode completely as accelerator
requests and are refused only by the program's own Direct-V2 count check.

Transaction extents ran 169–755 bytes against Solana's 1,232-byte packet
maximum. ProgramTest submits no packet and cannot enforce that limit — this is
the defect class Found31 hit — so the producer serialises each transaction
itself and a witness checks the measured extent.

### Two refusal codes that cannot be raised

`DirectAotSbfError::InvalidBank` (2) and `::InvalidAck` (3) are unreachable by
any input. The bank widths are validated against compile-time constants before
either decode runs, so `decode_register_bank_into` and `encode_register_bank_into`
cannot fail; `execute_atomic` can only return `RegisterWidthMismatch` on a width
already checked; and `InvalidAck` additionally needs SHA-256 to return
thirty-two zero bytes. They are recorded as never raised rather than
manufactured into a case.

### Fast lane, and it says so

`TIERS.md` permits `solana-program-test` to back a tier's fast lane only under
four conditions, and requires the tier to state which. The answers ride in the
evidence document's own `fast_lane` block, beside the numbers they qualify:

- **No Loader-v3 dependency.** The route authenticates no account at all and
  refuses any frame carrying one, so it cannot depend on genesis ProgramData
  layout, on `SetAuthority(Some -> None)`, or on deployment slots.
- **No packet-limit dependency.** 584 request bytes, empty account list; the
  producer measures the extent rather than asking the runtime to enforce it.
- **Compute and heap.** 1,400,000 CU set explicitly and never adjusted; the
  runtime default 32,768-byte heap, never raised. No diagnostic budget appears
  anywhere in this campaign.
- **Real account shapes.** Vacuous here by construction, and the one hostile
  case that presents an account presents a real ProgramTest System-owned payer.

The honest gap is **finality**: ProgramTest has none. `slot` orders the campaign
and proves nothing. A validator tier for this route is still owed.

## 2. The registered controller lifecycle, re-measured

`crates/dclutch-svm-harness/tests/physical_direct_composition.rs` runs the real
`controller-proof`, `claims-proof`, `custody-proof` and official SPL Token 9.0.0
ELFs under `solana-program-test`. Re-measured at `7123164`: **5 passed, 0
failed, 1 ignored** (the ignored one spawns an external `solana-test-validator`).

These numbers **supersede the W2e-era figures** that were still being quoted
(59,134–59,143 CU fills, 6,256 cancel) and the table in
`COMPILED_SIGNED_DIRECT_2026_08_25.md`. Everything drifted upward.

| route | case | CU |
|---|---|---:|
| registered create | unapproved | 31,370 |
| registered create | first creation | 48,663 |
| registered create | reused replay | 42,596 |
| registered create | wrong nonce, rollback | 42,477 |
| registered fill | wrong coordinate | 29,407 |
| registered fill | first residual | 66,769 |
| registered fill | terminal residual | 66,759 |
| registered fill | late rollback | 67,242 |
| registered terminal | stale cancel | 2,802 |
| registered terminal | impersonated cancel | 2,811 |
| registered terminal | **cancel** | **6,261** |
| registered terminal | replayed cancel | 2,794 |
| registered terminal | early expiry | 2,798 |
| registered terminal | **expiry** | **6,242** |
| registered terminal | replayed expiry | 2,613 |
| registered retire | open refusal | 3,022 |
| registered retire | seller retirement | 6,639 |
| registered retire | unsigned buyer | 4,910 |
| registered retire | buyer revoke + retirement | 10,359 |
| inline compiled | controller-PDA impersonation | 14 |
| inline compiled | wrong replay bump | 32,651 |
| inline compiled | wrong Position bump | 35,752 |
| inline compiled | wrong authority | 23,444 |
| inline compiled | price below signed seller limit | 42,341 |
| inline compiled | fee-rate byte tampered after signing | 0 |
| inline compiled | **admitted fill** | **65,741** |
| inline compiled | late rollback | 61,744 |
| address lookup table | create | 10,517 |
| address lookup table | extend (12 addresses) | 9,304 |
| address lookup table | deactivate | 3,151 |
| address lookup table | close (after 512 slots) | 2,158 |

Wire extents: registered fill 762 bytes legacy / 271 v0; inline compiled fill
1,326 legacy / 804 all-address v0 / 990 reusable-Market v0. All unchanged.

**The drift is real and worth naming.** The admitted inline fill was last
recorded at 59,037 CU and is now **65,741 (+6,704)**; late rollback moved
58,076 → 61,744 (+3,668). The registered fill's 59,134–59,143 became 66,769 /
66,759 (+7,6xx). Cancel moved 6,256 → 6,261 (+5). Nothing in this lane caused
that; the artifacts changed underneath the numbers, and the numbers were still
being quoted. That is the whole argument for a census: a measurement with no
campaign behind it decays silently.

### Why this flipped no census row

Ten `controller-proof/*` routes stay NEVER-EXECUTED, and the reason is narrow
and fixable: **the harness discards the finalized log messages.** It records
compute units for its own `eprintln!` and drops everything else. `census
observe` cross-checks every route claim against the chain's own
`Program <address> invoke [n]` lines and refuses a claim it cannot corroborate —
correctly, because a campaign that asserts its own coverage is a mirror. The
work is to thread a labelled recorder through that file's six submit helpers
and emit the evidence document; the campaign itself already exists and passes.
`tools/gauntlet/blocked.json` now says exactly this, and names an owner.

## 3. Direct through common Hot: where the wall actually is

Run at `7123164` against the same clean-HEAD ELFs:
`programs/dclutch-trading-sbf/program-test/tests/registry_hot_continuation.rs`
is **12 passed, 3 failed** — the same split W2g recorded, unchanged.

The three failures share one root cause. Trading panics inside
`process_hot_execution_v3` at 650,172–659,172 CU of the ~1,296,219 available,
and the runtime reports `InstructionError(1, ProgramFailedToComplete)`:

- `real_registry_executes_profile14_direct_hot_under_protocol_limit` — the
  admitted bundle never commits;
- `late_custody_refusal_rolls_back_registry_hot_claims_and_lifecycle` — fails
  its own honesty assertion, because the Claims children it claims to roll back
  never ran;
- `corrupt_live_profile14_maker_reserved_byte_refuses_without_mutation`.

Compute is **not** the blocker: the outer Registry continuation completes at
753,953–786,420 CU of 1,400,000, and the inner Trading invocation dies with
roughly 637,000 CU still unspent. This is a panic, not an exhaustion.

Everything before that point does execute, and already carries hostile cases:
the legacy-headered container, reordered Core/Trading roles, a substituted Core
ProgramData, altered Hot bytes, an aliased ephemeral admission, a corrupt root
reserved byte, and four capability-seal cases. Widening the hostile set *at the
phases that do execute* means editing that file, which is W2h's live surface —
so this lane measured it and left it alone. The phases past the panic are
W2h's gate and are not reachable for anyone yet.

`trading/*` is additionally **invisible to the census right now**: the
enumerator finds no `entrypoint!` in `programs/dclutch-trading-sbf/src/lib.rs`
at HEAD (`9abed0c` took the entrypoint vector back into a named machine
boundary), so Trading contributes zero routes and all five of its `blocked.json`
entries report as stale. That is an enumerator gap, not a coverage change, and
it belongs to whoever owns `census/src/enumerate.rs` next. No Direct row can be
claimed through Trading until it is closed.

## 4. The AOT differential

`crates/dclutch-direct-aot-v3-contract` compares its hand-written Rust AOT
translation against the Lean-emitted transition program executed by the generic
`dclutch-transition-vm` interpreter. The corpus went from 15 inputs to 171, and
the comparison itself was strengthened: both output banks are now checked on
refusal (previously only the AOT's), the two refused banks are compared to each
other, and the scratch banks are compared on admission.

Scratch on refusal stays deliberately unasserted. The contract promises only
that refusal leaves *output* unchanged, and that promise is load-bearing: on
the inline path the two executors' scratch banks diverge on 10 of 50 refusals,
because the AOT computes a group into locals and commits it whole while the
interpreter writes each destination as it goes.

**One genuine disagreement**, left failing rather than accommodated:

```
tests::outcome_between_tail_count_and_outcome_count
AOT returned Err(CheckFailed) but the interpreter returned Ok(())
```

`execute_inline_candidate` hand-writes `outcome >= tail_count -> CheckFailed`
(`crates/dclutch-direct-aot-v3-contract/src/lib.rs:242-245`). The emitted
`DIRECT_ORDINARY_PRELUDE_V3` has no such operation — it compares the selected
outcome against the projected *Market* outcome count and never against the
*Product tail count* that sizes the item registers. With `OUTCOME_COUNT = 5`,
outcome 4 and `tail_count = 3`, the interpreter admits a fill whose every item
claim quantity is zero: collateral moves, both nonces advance, no claims change
hands.

Qualified honestly: the encoder derives `tail_count` from
`context.outcome_count` (`crates/dclutch-direct-codec/src/ordinary_v3.rs:365`),
so on the emission path the two are equal by construction and the divergence is
unreachable — and nothing outside this crate's own tests calls either entry
point at all. If the clause is wanted it belongs in Lean; a semantic admission
guard existing only as hand-written Rust is the drift, not the fix.

Two further observations, recorded in the corpus rather than asserted: neither
program guards `fee_bps` against the denominator on the inline ordinary path
(10,001 bps is admitted by both, fee equal to gross), and `u64::MAX` rent
principals and root open count are admitted on the registered fill because those
registers are only nonzero-guarded and never enter arithmetic. The V2 descriptor
in `dclutch-direct-aot-contract` *does* check `policy_fee_bps <=
FEE_DENOMINATOR_V2`; the V3 inline path does not.

These are Rust case tests: two Rust executors over the same emitted program on a
fixed corpus. They are not translation validation, not a refinement proof, and
say nothing about inputs outside the corpus.

## 5. A build defect found on the way

`programs/dclutch-claims-proof-sbf`, the Direct capability-child claim executor,
did not build at all:

```
error[E0152]: found duplicate lang item `panic_impl`
  = note: the lang item is first defined in crate `std` (which `digest` depends on)
```

`crates/dclutch-product-payoff-v2-codec` is `#![no_std]` but declared `sha2`
with default features on. `sha2/default` pulls `sha2/std` pulls `digest/std`,
and resolver-2 unification put `std` into every graph reaching it; the program's
own `#[panic_handler]` then collided. Fixed in `7123164` by passing
`default-features = false`, the same way every other codec-layer sha2 consumer
already did. `claims_proof_sbf.so` now builds at 28,392 bytes with zero frame
diagnostics.

The same latent shape sits in eight other on-chain-reachable manifests and is
recorded on the wave board.

## Reproducing

```sh
# the Direct stateless AOT tier: archive -> build -> campaign -> census
tools/gauntlet/direct/run-direct.sh

# the registered controller lifecycle
SBF_OUT_DIR=$PWD/target/deploy cargo test \
  --manifest-path crates/dclutch-svm-harness/Cargo.toml \
  --test physical_direct_composition -- --nocapture --test-threads=1

# the AOT / interpreter differential
cargo test -p dclutch-direct-aot-v3-contract --lib

# Direct through common Hot (W2h's gate; 12 pass, 3 fail at this revision)
SBF_OUT_DIR=<clean-head-deploy> cargo test \
  --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
  --test registry_hot_continuation
```

## What this is not

Genesis-imported fixture accounts are not evidence for account-creation
workflows. `solana-program-test` is not a validator: it has no finalized
commitment and submits no packet. None of these numbers are devnet or mainnet
evidence. The Lean theorems named in
`COMPILED_SIGNED_DIRECT_2026_08_25.md` cover admission-to-abstract-VM behaviour
and do not extend to the Rust VM, the SBF ELFs, CPI, account decoding, or
Solana runtime behaviour — and nothing in this document changes that boundary.
