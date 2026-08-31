# The first Direct fill on a local validator

**2026-08-31, lane FILL-2.** The eighth stage executed. A Direct Hot trade
landed on a loopback validator, settled its collateral, and conserved every
atom it moved. This document is the one lane FINALIZATION deliberately did not
write, on the grounds that it belongs to whoever lands the fill.

It also records, in the same breath, the one thing the fill did NOT do: the
driver refused its own landed transaction's evidence on 32 bytes, for a reason
now understood, fixed, and verified by derivation rather than by a second fill.

## The transaction

```
signature  4hse1dNhpeN9h3LoYGFZCa53YjkQB1sgS5q6HPQaD9CBaXDKn6K2uHRzst8vMGbkGNmmJX1oiiHvNVY7jd7SCbhb
slot       7576
err        None
compute    1,282,624 of 1,399,700 requested
fee        15,000 lamports
market     7hRLVijyCd6FoGxAdPC1nsFAZ5VtN5kwvPvsaRMHLmky, generation 2, four cells
root       Dw15TRiqyELHVc7KtdonbsDxiaEsgpkQwW5Xjwv1u2Wm
validator  http://127.0.0.1:42888
```

The Trading program returned `DCLTHAK3` and succeeded. Inside it, Custody
returned `DCLCUDC2` and the Token-2022 `TransferChecked` moved the collateral.

## The whole route, every stage that landed

| stage | signature | slot | CU |
| --- | --- | --- | --- |
| replay setup | `5bYakKVABd5PQcc4Wanwhyv9Fr7uQc6z57hyjZqaico5` | 7291 | 189,109 |
| token setup | `KGjWw3KKCBA9D2zkU37jkYeuiHGbpTSuV2yRBkFbwZ8e` | 7327 | 114,632 |
| lookup create | `4MEKg36sKgaQZVcTmWhkPuQMJKNLF2cUzaxZAaxXDfF3` | 7362 | 10,409 |
| lookup extend | `4kwJhmQwbqEjxMxasumNGDBwbFt5DgkqTt4ppnucVaxj` | 7398 | 11,657 |
| lookup extend | `5sfmhDvG3oqMM9jK1tL7pXcYxj1qtdfxqde9pMrsSXRn` | 7434 | 11,660 |
| lookup extend | `4FEpf8HEXfvKXUUEy15566JTLL9wvKdnVXFx7DLqeSzG` | 7469 | 10,780 |
| lookup freeze | `aDNj2316djefqpMfMjSvZEquiXryB8LLQSCdhzje9vkm` | 7504 | 1,517 |
| lookup activation | (observational, no transaction) | 7506 | — |
| **capability seal** | `X9aSPWyfdWQTUh12Fg29exA1Yh9R74FsdLFo8dPAwqTk` | 7541 | **739,722** |
| **Hot execution** | `4hse1dNhpeN9h3LoYGFZCa53YjkQB1sgS5q6HPQaD9CB…` | **7576** | **1,282,624** |

## Wall 8 is verified

FINALIZATION's seal fix (`7623e436`) was committed and never sent to a chain.
It is verified here. The seal that previously refused `0x4008`
(`TradingSbfError::HeapFrame`) after consuming 24,033 CU of an allocation it had
not been granted now **finalizes at 739,722 CU**. The sentence in the old
comment said *allocation*, which is the heap, while the code granted compute
units; granting both is what the chain wanted.

One thing that had to be true first and nearly was not. The deployed
`trading.so` in `target/deploy/` predated commit `8c216642`, the CLOSESEAL
change that made a heap grant admissible on the seal outer at all. Verifying
wall 8 against that ELF would have proved nothing. Every program in this run was
rebuilt from source; the resulting `trading.so` is byte-identical
(`db600a71228fed6ecdcd7319576cf25b1fc95eb2b83747ee9f58da37fc471f3f`) to an
independent build another lane made at a different commit, which is as good a
determinism check as this tree offers.

## The conservation read-back

Read back from the chain rather than from the driver's journal, because the
journal records what the driver expected and the entire point of a read-back is
to ask a second source. Only the signature was taken from the driver.

By the transaction's own pre/post token-balance vectors:

| account | before | after | delta |
| --- | ---: | ---: | ---: |
| buyer collateral `Db41NLizFSVP…` | 50,250,000 | 500,000 | **−49,750,000** |
| seller destination `BkViVdhqnyBb…` | 0 | 49,750,000 | **+49,750,000** |
| fee destination `ESV3Uuk45kTW…` | 0 | 0 | 0 |
| | | **net** | **0** |

**Conserved.** Every atom debited from the buyer arrived in the seller
destination.

The fee is not missing and the zero is not a defect. It is accrued, not
transferred: the buyer's maker replay root carries `fee_owed = 500,000`, which
is exactly what the driver projected for it, and the fee token account's
poststate matched the projection byte for byte with its zero balance. A naive
gross/fee split predicted otherwise and was wrong; the protocol's own numbers
are the ones the chain and the driver agree on.

The buyer's remaining 500,000 balance still carries a 500,000 delegated
allowance to the Custody authority. The allowance is decremented by what it
spends rather than cleared, which is worth knowing before assuming a fill leaves
a buyer with no standing authorization.

## Wall 3 is verified: the allowance is an equality

The probe admitted 100,000,000 atoms against a 50,250,000 debit, so no fill
could ever land from it — FINALIZATION measured this and could not fix it
without a validator. It is fixed at its author (`322b54c5`) and verified here:
the admission landed with `quantityAtoms: 50250000`, the exact derived debit,
delegated to Custody authority `9kAR2xifXb293mvX18spycCWqoAh1CiFXtMbEKQTAjGS`.

The defect was that one number was doing two jobs.
`PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS` sizes the **bankroll** the founding mints
into the participant's source account, where a round 100,000,000 is right.
`--collateral-quantity-atoms` sets the **single-use authorization** over the
buyer's collateral, which the chain tests as an equality. `run.py` now derives
the second and asserts the produced manifest's `fill`, `executionPrice` and
`feeBasisPoints` against its own prediction, so a Rust-side drift breaks the
probe at the first moment both numbers exist.

## Wall 10, found by the fill itself

The driver refused the transaction it had just landed:

```
REFUSED: Direct finalized account GtYbZ8UrSKNSfhHSmFmqGNbowsTziBT1vLNXfHryb98q
         differed from its semantic-owner poststate
```

Of the ten poststates the Hot stage projects, **eight matched exactly** —
including all three token accounts, the Direct root, the Custody replay, and the
claims accounts. Both maker replay roots differed, in the same 32 bytes, at
`rent_owner`:

| | seller root | buyer root |
| --- | --- | --- |
| `next_nonce` | 1 = 1 | 1 = 1 |
| `live_count` | 0 = 0 | 0 = 0 |
| `rent_principal` | 2,004,480 = 2,004,480 | 2,004,480 = 2,004,480 |
| `fee_owed` | 0 = 0 | 500,000 = 500,000 |
| `rent_owner` **expected** | `6H8Ks96rr…` (payer) | `6H8Ks96rr…` (payer) |
| `rent_owner` **actual** | `3FhUmnZgN8…` | `3FhUmnZgN8…` |

`maker_facts_v1` projected the payer, under a comment that had already named
itself as the one coordinate to change if maker-root rent ever moved to
RentCredit. It moved and nothing followed it. `hot_v3.rs` builds
`MakerReplayFirstUseV1 { rent_owner: plan.beneficiary }`, and that beneficiary is
`credit.refund_wallet()` of the founding lifecycle RentCredit.

The chain is right and the producer was wrong, for a reason worth stating: a
maker replay root is a shared structure **of the market**. If its rent followed
whoever paid, a stranger paying their own fees would walk away owning the rent
of something the market depends on — and this same route deliberately admits
that stranger as the payer.

Fixed in `9d4935d2`. **Verified by derivation, not by a second fill**: the
lifecycle RentCredit `7BLy7beYaFq2LxSpZB5mxSJBNg74rSLjTZAFty5PuCSS` on this
chain carries refund wallet `3FhUmnZgN8dcfDXzVqNnHeQYof4bu4iz2MPS1jksjXDp` at
its `STATE_REFUND_WALLET_OFFSET`, which is exactly what both maker roots hold.
Confirming it end to end needs a fresh mutable substrate, because founding is
one-shot per substrate; that confirmation is the debt this fix carries.

## Wall 7, half down

The producer admitted only a vacant seller/fee token, so no market could trade
twice. It now admits both prestates the chain admits (`b7116f78`), names every
failing clause rather than the first, and names a half-run token setup — one
vacant destination and one initialized — as the thing no instruction can move
forward.

It deliberately does **not** require the balance, the lamports, or the delegate.
Token setup's poststate is a zero-balance account, and mirroring that here would
have built a fresh wall one trade further out: a market that trades exactly
twice instead of exactly once. The red-proof asserts separately that a seller
destination which has already been paid is still payable.

The other half is the setup stage machine's skip, and it is **not** landed.
Mapping it found a sixteen-edit change across a journal invariant that requires
a cryptographically verified signed packet at `Finalized`, the finalized
mutation evidence schema, and index arithmetic at fifteen refusal sites — plus a
divergence someone should decide deliberately rather than in passing, since the
driver's own token poststate predicate demands a **zero-balance** account and so
would refuse the very market the skip exists to serve. Until it lands, the
producer refuses that market by name at produce time, before signing two intents
and writing a session that could never advance.

## The ledger purge, closed on both launch paths

`solana-test-validator` keeps 10,000 shreds of root slots by default. The
drivers re-verify every earlier stage from transaction history on **every**
invocation, so a purge mid-sequence does not slow a campaign, it ends it:
`finalized Direct signature omitted transaction history`, with no resume,
because the evidence the next stage must cite no longer exists. It cost
FINALIZATION its sequence at slot ~46,000.

`ff2c8e35` fixed the probe's launch path. `c822a062` fixes the other one —
`dclutch-successor-validator`, which `bootstrap/successor`'s own `run --spec`
spawns and which still ran on the default. Both are pinned to 100,000,000
shreds, and the comments say so, because a campaign that survives on one launch
path and strands on the other is worse than one that strands on both: only one
of them teaches you anything.

**The retention is not free, and the justification written beside it is wrong.**
Both sites carry a comment reasoning that the cap costs nothing because "a
loopback session that lands a few thousand transactions writes a few tens of
megabytes whatever the cap is". Measured on this run: **5.9 GB of rocksdb at
slot 12,779**, roughly 470 KB per slot, on a session that landed well under a
hundred transactions. The bytes are the validator's own block and shred
bookkeeping, not this campaign's traffic, and under the 10,000-shred default
they would have been purged away — which is exactly the purging that ends
campaigns.

The fix is still right: a stranded sequence is unrecoverable and disk is not.
But the number is two orders of magnitude off the estimate, and a long campaign,
a small disk, or several concurrent lanes each holding their own ledger will
find that out. Whoever tunes this next should budget roughly half a gigabyte per
thousand slots, and consider retaining generously rather than unboundedly.

## Width

This fill is on a **four-cell** market, and the Hot stage's hard-pinned geometry
(`unique = static4 + loaded57 = 61`, `wire = 1,167 bytes`) **held** there.

That gate is worth a second look before anyone trusts it at another width. It is
three literals, and the account set they describe is not obviously width
independent: `pack_runtime` in `direct_inline_route_v3.rs` sizes the physical
account vector through `physical_account_count_with_dynamic_spans(outcome_count,
…)`, and the operator fixtures those numbers were measured against use
`outcome_count: 3`. It has never been exercised at more than one width, because
until now nothing reached the eighth stage at any width.

A six-cell market input compiles (4 cuts, 6 coefficients, 40,892 bytes). It was
not founded, because founding is one-shot per mutable substrate and this one had
already founded its market. **The second width remains unfilled.**

## The substrate, and how to resume it

Everything is on persistent disk, not `/private/tmp`, which is what the reboot
took from FINALIZATION.

```
/Users/ember/jobs/dclutch-fill2/
  src/                  clean git worktree pinned at 45e7adc0
  relwork/              checked release, gate 42882616f1d5a056…, 13 links, 0 diagnostics
  probe/                the run.py work root
    runs/seed-01/
      ledger/           THE VALIDATOR LEDGER
      mutable/keys/     every role keypair
      participant-handoff.json
  fill/session/         public manifest, private session, stage journals
  conservation.py       the read-back above, re-runnable
  stage.sh fill.sh market2.sh detach.py
```

The validator is `solana-test-validator` on RPC `127.0.0.1:42888`, held alive by
a `run.py` that has SIGSTOPped itself after the participant stage. **Its
watchdog kills the validator if that process dies**, so anything that wants this
chain must keep the stopped supervisor alive, or restart the validator against
`runs/seed-01/ledger` directly — a restart with the same ledger preserves all
account state.

The market is filled: `7hRLVijyCd6FoGxAdPC1nsFAZ5VtN5kwvPvsaRMHLmky`,
generation 2, four cells, one Direct trade settled at slot 7576. The Hot stage
journal is at `submitted` rather than `finalized` because of wall 10; the chain
state is complete regardless, and the fill is in transaction history.

## Not verified

- Wall 10's fix against a chain. Its derivation is verified; a second fill is not.
- The second width, at any stage past market compilation.
- A driver-*accepted* fill. This fill landed and conserved; the driver refused
  its evidence, for the one reason above.
- The token-setup skip, so a market still cannot trade twice.
- No devnet read or write of any kind.
