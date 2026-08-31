# Two of the three layers, and the frame cost that no build reports — 2026-08-31

Written by FRACCHECK-3, continuing FRACCHECK-2
(`docs/evidence/FRACTIONAL_COMPACTION_TRADING_HALF_2026_08_31.md`) and
`docs/design/CLAIM_CHECK_COMPACTION_V1.md` §17.4–§17.5.

## Result

**A fractional compaction request now reaches route selection.** The two layers
that gate whether a Claims route can be *reached* from Trading's Hot path —
composition decode and execution — admit `DCLTFCC1`, at a frame width that is a
number rather than a `~`. The route itself is still not written, and the
transaction stops at exactly one named place: a receipt verifier that refuses,
because no receipt type exists to verify against.

That is the honest state. Route selection is real; the route is not.

## What landed

| commit | what |
|---|---|
| `a6e8869c` | the crate's own `--all-targets` clippy red, in a test nobody lints |
| `0cb9c12b` | the 48-account frame: 36 + 12, named, with reasons for four refusals |
| `b198dfcf` | composition decode + execution, and the arm split out of a shared frame |
| this one | the evidence and the re-size |

### The 48 is 36 + 12, and the twelve have reasons

`FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 = 48`, built as
`FRACTIONAL_COMPACT_TERMINAL_FRAME_V1 (36) + FRACTIONAL_COMPACT_OWN_ACCOUNT_COUNT_V1 (12)`.

**The terminal half is not re-enumerated.** Its roles, order and privileges
belong to `terminal_settlement_v3`, which is the crate that decodes the header
this route carries verbatim, and the native crank wraps the same frame the same
way (`claim_check_compaction_v1::TERMINAL_FRAME_V1`). The design's rule about the
payout derivation — *call it, never re-implement it* — is the same rule one level
up, and a second enumeration here would have been a second author for one frame.

Six of the twelve are the native crank's own six, in the native crank's order:
escrow, the record it mints, the admission it reads the owner kind off, the
RentCredit, the opener it repays, the System program. The other six are what
§17.4's hand-off costs:

| role | why it is there |
|---|---|
| `FractionalCapabilityRoot` | the reserve Position's owner *and* the Mint's `PermissionedBurn` authority. Both signatures die with the market, which is the whole reason the hand-off happens at compaction |
| `TradingCallerAuthority` | the reserve Position's close is parent-authenticated. This pair **is** what "Trading-composed" means |
| `ShardMint` | the `SetAuthority` target, and the supply the record pins |
| `ShardTokenProgram` | separate from the terminal frame's Token program **on purpose** — that one is the *collateral* mint's, and folding the two works until the first market whose shards and collateral live under different Token programs |
| `ExposureTerms` | the denominator's sole author; the record persists it forever |
| `TokenBehavior` | the profile the split-controller reader runs is terms-selected |

**Four roles are declared and refused**, and `admission()` returns a *reason*
rather than a bool because the four are refused for three different reasons:

- `HolderShardTokens`, `HolderCollateralTokens` → **`RefusedNamesOneHolder`**.
  §1.3 — positions are never enumerated on chain — is exactly what lets *one*
  transaction stand in for every holder of a coordinate. A frame that grew a
  holder account would make this route per-holder, unbounded in transactions per
  coordinate, and would burn one holder's shards at a moment they did not choose.
- `NativeClaimCheckRecord` → **`RefusedUnsignablePayee`**. FRACR3's weld from the
  other side: a `TradingRecord`-owned Position is refused a native claim-check
  because its payee cannot sign, and this route exists *because* that refusal is
  right. Reaching for the native record inside it would undo the reason it was
  written.
- `RetirementCursor` → **`RefusedNotThisRoute`**. Compaction is permissionless
  and unordered; the cursor is ordered and stateful. Carrying it would let a
  stalled walk block a crank whose purpose is running when nobody is minding the
  market.

`index()` returns `Option` and is `None` for every refused role, so there is no
number to write down for an account this route must never reach for.

### The composition gap was not where the table put it

FRACCHECK-2's table names `composition_v3.rs::decode_selected_with_external` as
absent alongside `hot_v3.rs`. **`decode_selected_with_external` needed no edit at
all.** It already admits any caller-authenticated external request at an exact
fixed account count, already refuses a substituted request, a wrong geometry, a
borrowed witness and a second external route, and already counts the admitted
route as the single mutation.

The gap was one level up and narrower than stated: nobody ever *built* a
`ClaimsExternalOnceV3` for anything but the exposure magic. The generic mechanism
was finished; the caller was the hole. Edits needed in `dclutch-claims-svm` for
this layer: **zero**.

Worth recording because the size of a gap and the size of the machinery around it
are different numbers, and the table conflated them.

### The frame cost, measured — and no build reports it

This is the finding worth carrying forward.

A fractional compaction request is **744 bytes** and decodes into a struct that
embeds the whole `TerminalSettlementRequestV3`. Written as an ordinary `else if`
arm inside `route_authority`, that struct joins the frame **every other Claims
route also pays for**:

| function | base | inlined arm | split out |
|---|---:|---:|---:|
| `claims_composition_v3::route_authority` | 3,072 | **3,712** | 3,072 |
| — its spare | 1,024 | **384** | 1,024 |
| `fractional_compaction_authority` | — | — | 2,176 (1,920 spare) |
| `hot_v3::decode_claims_composition_boxed_v3` | 3,200 | 3,200 | 3,200 |
| deepest frame in the link | 4,032 | 4,032 | 4,032 |

**`cargo build-sbf` reported zero diagnostics at 3,712 exactly as it does at
3,072.** The diagnostic only fires at or past 4,096, so a 640-byte jump that ate
62% of a function's remaining headroom was completely invisible to the gate
`tools/ci/run.sh` runs — on a link whose deepest function already sits 64 bytes
from the wall. This is precisely the blindness `tools/sbf-frame-sizes.py` was
written for, and it is the first time this lane family has watched it happen
prospectively rather than in a post-mortem.

`#[inline(never)]` on the arm returns `route_authority` to its exact base frame.
The baseline is a real build of this lane's own base commit (`93329f90`) in a
separate worktree, not a remembered number.

**The generalizable rule, for whoever writes commit 6:** any arm that decodes a
wire embedding the terminal header must be split behind `#[inline(never)]`, and
the split must be *measured*, not assumed. The Claims route will decode the same
744-byte request inside a 48-account frame, and `custody_replay_v1::process`
already holds the claims-sbf link at 3,776 of 4,096 — 320 spare.

### A mutation found a hole in this lane's own test

The frame's *order* is load-bearing: a caller builds the account vector from it
and the route reads by index. The first consecutive-index test derived its
expectation from `frame()` itself, so it held under **any** permutation —
swapping two entries left it green. The order is now spelled out literally and
the same swap reds it.

Recorded because the test's name (`..._never_collide`) described a property it
did not have, and only running the mutation could tell.

## The receipt is still deliberately absent, and the arm refuses

`verify_route_receipt` gains an arm for the new kind that **returns an error**.
It is the only arm there that does, and it is correct rather than a stub:

- Admitting the route unverified would make "verified" mean "unchecked" for one
  kind, which is the shape a reader cannot see.
- Inventing a receipt now would fix a field set to what a route that does not
  exist is *guessed* to produce. FRACCHECK-2 declined for that reason, and being
  the lane that will write the route does not make the guess safer — only more
  convincing.

A test asserts the refusal, so a later lane that adds a receipt and forgets this
arm fails here rather than shipping a vacuous verification.

The post-resource evidence arm is written anyway, explicitly rather than
inheriting the wildcard's `None`. The evidence is unread today; writing it now is
what stops a later receipt from being checked against nothing while still looking
verified.

## The re-size

FRACCHECK-2: **seventeen commits, eight landed, nine remaining.** Two corrections
move the total, both upward, and both are surface rather than route code.

**The frame declaration was inside commit 6 and is separable.** It is what all
three layers consume, so building it first is what let the composition arms name
a constant instead of a literal. Call it 5a; it has landed.

**5c is two commits, not one.** The re-size costs it as "`action_geometry`,
`encode_effect`, `encode_account_profile`, plus a `FractionalFrameKindV3`
lock-count row". That understates it. `encode_request_profile`
(`artifacts_v4.rs:569-657`) pins `FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2` at offset
0 and the action byte at `FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2` into the
profile itself. A compaction carries a different magic at a different width with
different offsets, so **it needs its own request profile**, not a fourth arm in a
geometry match. The account profile and effect program follow the same shape.

| # | commit | status |
|---|---|---|
| 1–4, 5d, 10a, 12 | FRACCHECK + FRACCHECK-2's eight | **landed** |
| **5a** | **new** — the 48-account frame, declared | **landed** (`0cb9c12b`) |
| **5b** | the composition-decode arm | **landed** (`b198dfcf`) |
| **5** | the execution arm | **landed** (`b198dfcf`), receipt arm refusing |
| 5c-i | **new split** — a request profile for the compaction wire | not written |
| 5c-ii | `action_geometry` / `encode_effect` / `encode_account_profile` + the lock-count row | not written |
| 6 | the Claims compaction route | not written — **still a lane** |
| 6b | **new split** — the receipt type, and the Trading verifier arm it turns green | not written |
| 7 | the burn-and-pay redemption route | not written |
| 8 | `RetireCoordinate`'s compacted arm | not written |
| 9 | the fractional escrow close | not written |
| 10 | the route campaign | not written |
| 11 | the operator surface | not written |

**Nineteen commits, of which eleven have landed. Eight remain.**

### Commit 6, sized exactly

Every piece it needs now exists and has been read. It is assembly and
authentication, not invention — which is why it can be sized rather than
estimated:

| piece | exists? | where |
|---|---|---|
| the 48-account frame | **yes** | `FractionalCompactionRoleV1` |
| refusal codes `0x5640`–`0x564C` | **yes** | `fractional_claim_check_v1.rs` (sbf), 13 variants |
| owner-kind gate | **yes** | `owner_kind_may_open_a_fractional_claim_check` |
| conservation plan | **yes** | `FractionalClaimCheckCompactionPlanV1`, incl. `RateNotCovered` |
| the record | **yes** | `FractionalClaimCheckV1`, seeds keyed by the shard Mint |
| the payout derivation | **yes** | `terminal_settlement_v3::execute_claim_check_compaction`, callable verbatim |
| escrow/vault seeds | **yes** | `ClaimCheckEscrowSeedsV1`, `ClaimCheckVaultSeedsV1` |
| the split-controller reader | **yes** | `read_compacted_shard_mint` |
| the hand-off leg, proven on real bytes | **yes** | `escrow_pda_handover.rs` |

What has to be *written*: the frame guard and authentication walk (escrow,
deadline, vault derivation, record vacancy, admission owner kind **and**
`position_owner == root`, terms, token behavior, the pre-hand-off `read_mint`);
the `SetAuthority` CPI and the post-hand-off `read_compacted_shard_mint` check;
the plan, the record write, the escrow increment, the close-and-split; the
dispatch arm. The native route is 1,190 lines and this one carries six more
accounts and one CPI leg.

**The three hazards, named so the next lane does not rediscover them:**

1. **Frame size.** `claims-sbf`'s deepest is 3,776 of 4,096. The route decodes a
   744-byte request; do it behind `#[inline(never)]` returning `Box`, as
   `authenticate_compaction` does, and measure with `tools/sbf-frame-sizes.py`
   before trusting a zero-diagnostic build.
2. **The native helpers are private.** `write_claim_check`, `close_and_split`,
   `allocate_and_assign`, `observation` and `token_balance` all live inside
   `claim_check_compaction_v1` without `pub`. Sharing them beats copying them,
   and copying `close_and_split` in particular would put the amended lamport
   order (rent, **crank**, opener debt, residue) under two authors.
3. **The root signs, but Claims cannot sign for it.** The root is a Trading PDA;
   its signature arrives because Trading's `fractional_root_signer` adds it. The
   Claims route requires `is_signer` and never tries to produce it — the same
   shape `escrow_pda_handover.rs` proved, from the other side.

## Not verified

- **No Claims route was built**, so no route CU was measured and §17.3's ~928k
  projection remains a lower bound on a route that does not exist.
- **The composition arms are not driven end to end.** They are proved by unit
  tests over real encoded requests — route selection, the authority context, the
  refused route kind, the refusing receipt — and by the widened enum forcing four
  sites to answer. No program-test drives a compaction, because there is nothing
  to drive.
- **The 48 is a declaration, not an observation.** Nothing has yet built a
  48-account transaction; the number is derived from the terminal frame's own
  constant plus twelve roles argued one at a time. The campaign is what would
  turn it into an observation, and the campaign is commit 10.
- **`ClaimsCapability` is still stranded**, unchanged.
- **The remainder still goes nowhere**, as ruled: dust stays escrowed against a
  claim two holders can still consolidate to form, and no residual beneficiary
  exists to sweep it to.
- **No devnet write.** Both links rebuilt with zero SBF frame diagnostics; the
  Claims link's frames are byte-identical to base, and the Trading link's deepest
  frame is the same function at the same 4,032 bytes.
