# Decision 0022: a callee invoked by a PDA-signed CPI from Trading takes the facts the signer's seeds pin as established

Status: **PROVISIONAL — ruled by the orchestrator on 2026-09-02 under ember's
standing goal, spent the same day in Claims, Custody and the accelerator
prelude, and reversible by ember at the cost §7 states**. The ruling is
`GOAL.md:3678-3681`, carrying the standing formula *"RULING (under the standing
goal; ember may reverse)"*, and is explicitly *"decision 0012's argument one
level down"*. Landed at `0aa70478e` (Claims), `9b5de611e` (Custody),
`742d7b7be` (the prelude move), with `30d02f5c0`, `93120acfc` and frame rows
`fa00e8f28`.

## 1. The question

A completing SignedDelta child spent **149,107 of 173,680 CU — 85.9% —
re-authenticating what Trading authenticated in the same instruction**, and 662
CU (0.4%) applying the deltas it exists to apply (`b97ef3e4a`, quoted at
`GOAL.md:3866-3869`). *"The Remove's child has never executed a single delta"* —
two days of a route being called compute-bound while its whole budget went to a
second opinion about facts its caller had already established.

Custody measured the same shape independently: **77–81% of an invocation is
caller re-authentication**, against a Token-2022 CPI that costs 105 CU
(`GOAL.md:3691-3693`).

## 2. The ruling, verbatim

> **RULING (under the standing goal; ember may reverse): a callee invoked by a
> PDA-signed CPI from Trading takes the facts the signer's seeds pin as
> established** — the release set, the role activation, the sealed records —
> verifying only the signer's derivation; the unpinned-caller history stays as a
> hostile. Decision 0012's argument one level down.
> — `GOAL.md:3678-3681`

## 3. What it changed in the trust model — both halves, stated

The commit spells out the boundary in the two directions that matter, and the
second is why this is not a weakening.

**What the seeds establish** (`0aa70478e`):

> Nothing reaches this route without a `CallerAuthoritySeedsV1` PDA signature,
> and a program-derived address has no private key, so the signature is a
> statement by the program at the caller coordinate that it is the one invoking.
> Its seed order pins the release set, the Market, the caller's execution role,
> the replay context and `hash(instruction_data)` — the EXACT plan bytes

which is why `product_record_digest` and `linked_basis_record_digest` are
established for exactly what they name.

**What the seeds do NOT establish, and is therefore still checked**
(`0aa70478e`):

> Which program holds a role in that release set: the seeds name a role, not a
> key, and any deployed program can sign a PDA under itself. That is the hazard
> the old comment recorded and it is not repaired by the ruling

so every coordinate is still pinned — now against the program the Registry's own
activation names. Two conjuncts a signature cannot carry stay **unconditional**:
`authenticate_core_market_v3`
(`programs/dclutch-claims-sbf/src/signed_delta_v3.rs:414`), *"because a caller
may pin its own plan to whatever it likes but may not author the Market's
persisted principal cap"*, and `authenticate_market`, the Claims-owned Market
PDA.

## 4. What it saved, measured — with the saving split against itself

Deterministic spans, identical to the digit across independent runs with
different key draws (`0aa70478e`):

| span | before | after |
| --- | ---: | ---: |
| `SignedDeltaAccountsV3::parse` | 31,054 | 21,754 |
| `authenticate_releases` | 76,245 | 30,828 |
| `authenticate_product_and_basis` | 41,808 | 3,375 |
| a completing SignedDelta invocation | 173,676 | **80,488** |

**The Remove's Claims route now executes and commits.** The partial equity
Remove's SignedDelta child entered with 94,423 CU, applied its deltas, wrote its
candidates, set return data and handed 12,210 CU back — and the transaction
reached Custody's second route for the first time (`GOAL.md:3684-3685`).

**The lane split the saving against its own ruling** (`GOAL.md:3685-3686`):
*"45k was a redundant triple hostile-decode of one immutable account needing no
ruling, 38k is the ruling's."* `authenticate_releases` had called
`authenticate_activated_role` three times, each running the complete five-role
projection to answer one question. The commit states the standard: *"a
measurement that lets a ruling take credit for a redundancy is not evidence for
the ruling"*. A later claim was corrected the same way: *"93% of Custody's
`cf-accounts` was the DECODER, the ruling already spent at `9b5de611e`"*
(`GOAL.md:3803`).

Six frame coordinates stopped being bound — the input to an eight-derivation
Product runtime walk that is gone — which is the parse saving; they stay in
`SignedDeltaFrameSpecV3` because the frame is a wire contract shared with
callers, and `authenticate_privileges` still takes every coordinate's privileges
by index, so an unread account is still a refused writable or signer.

## 5. The hostiles that guard it

Three, in the real-ELF fractional SignedDelta program-test, **each proven to
reach its subject rather than asserted to** (`0aa70478e`) — and `Release` is one
discriminant over both the authority derivation and the release bind, so an
assertion on it alone would prove nothing about where the refusal happened:

- `a_caller_that_is_not_the_activated_trading_refuses_by_name` — a second
  deployment of the identical caller ELF holds the Registry's Trading
  activation; the test caller invokes, signs its own correctly-seeded PDA, and
  refuses **`0x5202`** (`programs/dclutch-claims-sbf/src/signed_delta_v3.rs:134`).
- `a_cache_for_another_release_set_refuses_by_name` — a complete,
  Registry-owned, canonically-addressed activation cache for a different
  generation. **`0x5202`**.
- `an_unsigned_caller_authority_refuses_by_name` — the activated caller invokes
  with `invoke` where `invoke_signed` belongs, *"the negative control the whole
  ruling rests on"*. **`0x5201`** (`:132`), from the frame spec's readonly
  SIGNER pin.

`GOAL.md:3687-3688` records why they are two codes and not one: *"hostiles
0x5201/0x5202 each proven to reach their subject (a shared code would have proved
nothing)"*. The ruling's own text retains the unpinned-caller history as a
hostile (`GOAL.md:3680`).

**A control that caught itself:** a suite that read 48 FAILED *"had not run (a
wrong Token-2022 build refused by the fixture digest)"* (`GOAL.md:3688-3689`).

## 6. What was given up, named as debt

The per-role deployment observation was **also the slot pin** — decision 0012's
`ReleaseSuperseded`, raised when the substrate's upgrade authority ships new
bytes under an open market. *"This route now inherits that refusal from its
caller, which observes all five roles before composing the child. It is not lost
from the transaction; it is lost from this program"* (`0aa70478e`). That is the
exact shape of the ruling: a fact moves from being re-established by the callee
to being established once by a signer whose derivation the callee still checks.

## 7. The cost of reversal

The Remove's Claims child stops fitting — it had never executed a delta before
this — and Custody's second route becomes unreachable again. Roughly 38,000 CU
per SignedDelta invocation returns, on a route already measured over budget, and
Custody re-acquires its 77–81% caller-re-authentication share. The 45,000 CU of
redundant decode is separable and would not return with it: that repair stands
on its own.

## Evidence pointers

`GOAL.md:3678-3693`, `:3803`, `:3866-3869`; commits `0aa70478e`, `9b5de611e`,
`742d7b7be`, `30d02f5c0`, `93120acfc`, `fa00e8f28`, `b97ef3e4a`;
`programs/dclutch-claims-sbf/src/signed_delta_v3.rs:132`, `:134`, `:414`;
`crates/dclutch-cu-checkpoint`;
`docs/decisions/0012-devnet-iteration-substrate.md`.
