# The Claims and Custody family campaigns

Real first-party ELFs and the runtime's real SPL Token and Token-2022 programs,
executed by `solana-program-test` rather than by a validator. Chain state — the
Core Market, the Registry activation cache, the Product Runtime V3 record graph,
the LiabilityBasisV2 aggregate, the Realm record, the token accounts, the
lifecycle RentCredit — is **installed directly**. There is no Hot execution and
no open Market in the loop, which is precisely why these routes are reachable
here and are not reachable in tier 1.

Two census campaigns, because a census campaign has one program map and the two
families pin different addresses for `registry`:

| campaign | source |
|---|---|
| `claims-family-programtest` | `programs/dclutch-claims-sbf/program-test/protocol-position/tests/lifecycle.rs`, `programs/dclutch-claims-sbf/program-test/sparse-chain/tests/sparse_chain.rs` |
| `custody-family-programtest` | `programs/dclutch-custody-sbf/tests/program_test.rs` |

Run the lane with `run-claims-custody.sh`; run any campaign on its own with its
`run-program-test.sh`. Both are ordinary tests when
`DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` is unset.

## What the campaigns drive

**Claims.** The canonical 26-account Admit and 15-account Close, and then the
composition commit `78bda05` describes and no live route can reach: Admit the
destination Position, carry its exact 512-byte typed receipt into a
26/22/15-account SparseNativeTransfer, and carry that transfer's exact 448-byte
receipt into the Close of the drained source Position — **one transaction, three
Claims routes, 537,855 CU**. Hostile cases: a Position owner the caller
authority does not authorize, an admitted Position re-admitted as vacant, a
zero-quantity transfer built as bytes because the codec refuses to construct
one, an admission receipt that decodes but names another owner, and a caller
that refuses after all three stages returned.

**Custody.** InitializeReplay, OpenVault, the ordinary Transfer, CloseVault and
CloseReplay, plus the DCLCUDQ2 delegated wire, run end to end twice: once
against SPL Token and once against Token-2022. Hostile cases: a replay close
under a live vault, a V1 external debit with no delegate, a substituted
delegate, a stale replay revision, substituted semantic owners, a foreign caller
role, a vault reopen after the replay is retired, and a caller that refuses
after the token effect.

## What this lane evidences

- The real ELF refuses and accepts exactly what its taxonomy says. Every named
  refusal in the bindings is compared by the census against the
  `custom program error: 0x…` the runtime itself logged.
- Rollback is byte-exact. Each campaign compares whole `Account` values —
  lamports, owner and data — before and after a refused transaction. The census
  does not model this; the campaigns assert it directly.
- Compute stays under the real 1,400,000-unit maximum with the real 32 KB heap.
  Both are gates here, not adjustable diagnostics: `set_compute_max_units` is
  called with exactly the runtime maximum and nothing requests a heap frame.

## What this lane does NOT evidence, and tier 1 must

Stated plainly, because `TIERS.md`'s fast-lane bar demands it. Of its four
conditions this lane satisfies three and **fails the first**:

1. **Deployment — NOT satisfied.** The campaigns INSTALL Loader-v3 ProgramData
   accounts with the documented layout instead of deploying and revoking. A
   layout the campaign wrote cannot corroborate a layout the loader would have
   written, and nothing here proves `SetAuthority(Some -> None)` or ProgramData
   deployment slots. Tier 1 owns that.
2. **Packet serialisation — satisfied by MEASUREMENT, not by enforcement.**
   ProgramTest submits no packet, so it cannot enforce Solana's 1,232-byte
   legacy maximum and a frame past it would survive untouched. Every campaign
   here therefore serialises each transaction it submits, records the extent as
   `wire_bytes`, and asserts it at submission; two witnesses check that the
   largest extent fits and that NO transaction went unmeasured. Every campaign
   transaction is a v0 message over a real, activated Address Lookup Table,
   which is the shape a validator would need.

   **Two packet defects surfaced the moment the measurement went in.** The
   composed Claims chain, with three 320-byte requests inline, serialised to
   1,261 bytes -- 29 past the maximum -- so the wrapper now DERIVES the Close
   request from the two requests the composition already binds and carries only
   the source Position's four rent facts, bringing the chain to 973 bytes. Any
   real controller composing these three routes has the same budget problem and
   the same way out: the join rules make the third request almost entirely
   redundant.

   **And the Custody family's own defect.** With keys
   inline as legacy messages, thirteen of the seventeen Custody transactions per
   token profile are past the maximum: OpenVault at 1,340 bytes (+108), Transfer
   and CloseVault at 1,306 (+74), and the DCLCUDQ2 delegated wire — a 776-byte
   request — at 1,410 (+178). Only CloseReplay (1,174) and InitializeReplay
   (1,208) fit. Nothing had ever noticed, because no tier had ever submitted a
   Custody transaction as a packet. The campaign now routes every transaction
   over a finalized lookup table as a v0 message, which brings the largest to
   1,043 bytes; any live caller of these routes must do the same, exactly as
   Found31 must.
3. **Compute and heap — satisfied.** 1,400,000 and 32,768, neither adjusted.
4. **Real account shapes — satisfied.** The real System program with its
   NativeLoader metadata, real SPL Token and Token-2022 mints and accounts
   packed by `spl-token-interface`, real Registry activation-cache bytes
   produced by the Registry contract's own encoder.

Two further limits worth stating: warping slots is not the passage of time and
nothing here pays a real fee market, and there is no concurrency, no fork and no
replay of a real ledger.

## Compute budgets

Compute figures from these campaigns **used to** vary by a few thousand units
between runs, because PDA bump-seed search cost depends on the addresses and the
Custody fixture drew four of them from `Pubkey::new_unique()` — a
process-global counter, read by two profile campaigns running concurrently, so
the draw depended on thread interleaving.

Those four now derive from a fixed seed hashed from
`dclutch/gauntlet/claims-custody/custody-program-test/keypair-seed/v1`, with the
profile label inside the derivation so the two campaigns still get distinct
addresses. The Claims campaigns never drew a random address.

**Seeding the four addresses was not enough, and the measurement is why we know.**
Two runs at one revision: Claims agreed on all 15 transactions, Custody differed
on **6 of 34**, every delta an exact multiple of 1,500. What was still random was
not an address — it was `context.payer`, ProgramTest's own genesis mint keypair,
which has no public knob to seed and which goes into `CustodyRequestV1.payer`
and therefore into the replay and vault derivations. The campaign now signs with
a SEEDED protocol payer and leaves `context.payer` as the FEE payer, where it
enters no derivation. That is the same split `found_program_test.rs` uses.

Both campaigns carry enforced CU budget rows in
`tools/gauntlet/CU_BUDGETS.json` — see `CU_BUDGETS.md` for the tolerance rule
and for the half of this that seeding does not fix.

**Budgets name LITERAL transaction labels; bindings use wildcards.** A binding
may say `custody *: open vault` because the census matches a family of
transactions; a budget must match **exactly one** transaction or the evaluator
returns `AMBIGUOUS`. So the budget rows name `custody legacy: open vault` and
`custody token-2022: open vault` separately. When you add a transaction to these
campaigns, the label you pass to the recorder is the string a budget will have
to name.

Even so, treat a number from this lane as a measurement with the 1,400,000 gate
on top of it, not as a constant of the protocol.

## One protocol observation the campaign records rather than asserts

The Claims sparse adapter treats the admission-receipt suffix as **optional**:
omitting it entirely skips the join and the transfer still commits. The exact
backward receipt dependency of `78bda05` is enforced by `ClaimsCompositionV3` in
the outer controller, which no live route reaches yet. The hostile case this
campaign drives is therefore a receipt that decodes and does not join — the
check `validate_sparse_admission_receipt_v3` actually owns — and the binding for
it says so.

## Refusals raised outside the census

Both families prove rollback with a test-only caller that refuses AFTER the
child committed, and the chain then reports THAT program's code. That number
used to collide with a first-party refusal it had nothing to do with — Custom(3)
was the caller's `DeliberateLateFailure` and also `claims/ClaimsSbfError::
Release` and `custody/CustodySbfError::CallerAuthority` — and crediting the
collision would have made the census claim a refusal that never happened. Bands
ended that (`docs/decisions/0007-namespaced-refusal-codes.md`): the Position
lifecycle's caller raises `0x102003`, the sparse chain's raises `0x105003`, the
Custody caller raises `0x107003`, and every one of those bands is above
`0x100000`, where no deployed program can reach. What bands did not change is
that a test caller is not an enumerated program, so there is no taxonomy entry
to credit its refusal to. Those bindings therefore still use
`unnamed_refusal { code, reason }`: the code is still checked against the chain,
and no first-party taxonomy is credited.
