# Decision 0023: a caller authority's address is a function of the signed instruction alone, never of the executing slot

Status: **PROVISIONAL — ruled by the orchestrator on 2026-09-03 under ember's
standing goal, landed the same morning, and reversible by ember at the cost §7
states**. The ruling is `GOAL.md:3874-3877`, carrying the standing formula
*"RULING (under the standing goal; ember may reverse)"*. Landed at `3a8ac205d`
(2026-09-03 07:28); the design note is
`docs/design/GENERAL_CALLER_AUTHORITY_SLOT_BINDING_2026_09_03.md` (`75215937f`).
It is a Trading change, so it rides to chain with cohort-15.

## 1. The defect

The read-only `devnet-general-session` driver (`d2d342573`) attributed all 55
top-level accounts of a General execution to an author, and found four that had
none. `GOAL.md:3866-3872`:

> the four caller authorities, which are **unstateable**: seeded from
> `sha256(request header ‖ inline bank)` while the bank carries `CURRENT_SLOT`
> from `Clock::get()` every execution — the address is a function of the
> executing slot and the account list is fixed at signing

A caller-authority PDA has to be NAMED in the top-level account list, which is
fixed when the transaction is signed. Those addresses were therefore valid for
exactly one slot and no caller could deliver into them: `0x4001` at the family's
entrance.

**The tree had already stated the law and not applied it here.**
`the_window_gated_actions_declare_the_current_slot_in_their_bank` says
*"Anything outside the executing instruction that has to STATE that bank is
therefore valid for exactly one slot, which no caller can deliver into"* — the
sentence that deleted the input scratch-page transport. These addresses survived
that cut *"because it reasoned about page ACCOUNTS and this is a page-less
ADDRESS"* (`shadow_digest_v3.rs:82-87`).

## 2. The ruling, verbatim

> **RULING (under the standing goal; ember may reverse): a caller authority's
> address is a function of the signed instruction alone, never of the executing
> slot** — `role_request_digest` becomes a slot-free digest
> (`sha256(parent_request_digest ‖ chunk_index)`); no trusted-environment scalar
> enters any address seed.
> — `GOAL.md:3874-3877`

## 3. What it changed in the trust model

**One preimage, one author, both routes**
(`crates/dclutch-execution-strategy-contract/src/shadow_digest_v3.rs:107`,
documented at `:66`, domain constant at `:28`, twenty-seven references across
eight consumers):

```
accelerator_caller_authority_digest_v1(kind, parent_request_digest, index)
  = sha256(b"dclutch:accelerator-caller-authority:v1"
           ‖ [kind as u8] ‖ parent_request_digest ‖ index.to_le_bytes())
```

`parent_request_digest` is `family_request_digest_v3` of the exact signed
`DCLTHOT3` payload — the value both invocation contexts already carry and every
reader already re-derives — and `index` is the invocation ordinal, the only
coordinate that varies between the invocations of one execution. All three are
computable by any caller that can build the transaction at all, and none is a
trusted-environment observation.

The domain separator and the `kind` byte are **additions to the ruling's simpler
`sha256(parent ‖ chunk_index)`**, and the commit says why: *"every other digest
in this tree is domain-separated, and the byte keeps the two accelerator
dispositions from minting one address rather than leaving that an argument about
`resolve_execution_candidate_v2` that has to stay true"* (`3a8ac205d`).

**Two routes had the defect, not one.** `shadow_composition_v3.rs` seeded from
`hash(ShadowRequestV3)`, which carries `digests.interpreted_candidate` over the
whole post-transition register bank — the bank `require_trusted_environment_v3`
pins `Clock::get().slot` into. *"Nothing pairs `ShadowAot` with a slot-declaring
profile today (Series declares `TrustedEnvironmentV2::None`), so it was latent,
and nothing enforces that pairing either"* (`3a8ac205d`). Fixed under the same
author at the same cost: *"zero now, and not zero after the first family needs
it."* The unenforced pairing later got a refusal of its own,
`ShadowTrustedEnvironment 0x4028`
(`programs/dclutch-trading-sbf/src/lib.rs:530`).

## 4. What it saved

Not CU — **statability**. Before the change General could not execute on a real
chain at all: the account list is fixed at signing and the address moved with the
slot, so the family refused `0x4001` at its entrance. After it, the ladder runs
N=2 at 603,939 CU, N=13 at 609,097, N=258 at 619,393 (`GOAL.md:3921-3923`).

## 5. The hostiles that guard it

**The two-slot proof**, in the General-hot suite
(`programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs:1190`):
`one_signed_account_list_opens_the_same_batch_at_two_execution_slots` builds ONE
`OpenBatch` host frame, executes it in two banks warped 47 slots apart, and
asserts the 55-entry top-level account list **byte-identical**. Under the old
seed four of those 55 entries would differ.

> The executed `Clock` is read out of both banks and asserted DIFFERENT first,
> because "nothing moved" and "my instrument was disconnected" log identically.
> — `3a8ac205d`

The unit twin needs no validator
(`programs/dclutch-trading-sbf/program-test/bundle-builder/src/admitted.rs:823`):
`one_family_request_names_one_authority_set_at_every_register_bank` derives the
authority span twice from banks differing only at `scalar::CURRENT_SLOT` and
requires the accelerator request digests to DIFFER while the authorities are
equal.

**Five substitutions, each shown first to name a different address** than the
honest one, in
`accelerator_caller_token_binds_request_context_and_immutable_deployment`: an
authority derived from the old slot-bound `hash(request_bytes)`, one for another
market, one for another release set, one for another signed family request, and
one for another chunk ordinal — all refusing `TradingSbfError::Release`. In
`shadow_composition_v3`, the test that asserted the OPPOSITE property — that
appending one byte to the request moved the address — is replaced, *"that
property WAS the wall"*.

The proof went green at HEAD (`GOAL.md:3921-3923`, slots 1 and 48, 603,939 CU
both) only after a separate defect was convicted: the admitted CPI loop paid for
`StableInstruction::from(instruction.clone())` once per chunk and exhausted the
heap with eight bytes of the 65,536 grant left. And the proof had previously
*"asserted with random keypairs at two top-level coordinates and could never have
proven it"* — the instrument was checked before the reading was believed.

## 6. What was given up, named

The authority is no longer bound to the exact accelerator request **bytes**, only
to the family request that determines them. What still covers that
(`shadow_digest_v3.rs:96-106`): everything else in those bytes is derived by
Trading from authenticated artifacts and chain state inside the same
instruction, and the callee re-derives it — the accelerators' own frame checks
and `require_admitted_bank_matches_frame_v3` cover the bank, and each
acknowledgement still names the digest of the request it answered, so a reply to
another request is still refused. *"The authority stops being a second,
redundant statement of what Trading just computed and becomes a statement of
what the caller asked for."*

## 7. The cost of reversal

General cannot execute on a real chain. The account list is fixed at signing and
the address moves with the slot, so the family refuses `0x4001` at its entrance
— which is the state the tree was in for its whole history until this commit.
The latent Shadow instance returns and re-arms the moment any family pairs
`ShadowAot` with a slot-declaring profile, which nothing enforces against.

## Evidence pointers

`GOAL.md:3866-3878`, `:3903-3905`, `:3921-3923`; commits `3a8ac205d`,
`75215937f`, `d2d342573`, `7a18a2272`;
`crates/dclutch-execution-strategy-contract/src/shadow_digest_v3.rs:28`,
`:66-106`, `:107`;
`programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs:1190`;
`programs/dclutch-trading-sbf/program-test/bundle-builder/src/admitted.rs:823`;
`programs/dclutch-trading-sbf/src/hot_v3.rs:14231`;
`programs/dclutch-trading-sbf/src/lib.rs:530`;
`docs/design/GENERAL_CALLER_AUTHORITY_SLOT_BINDING_2026_09_03.md`.
