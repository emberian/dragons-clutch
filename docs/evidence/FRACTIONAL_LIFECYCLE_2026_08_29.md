# Fractional lifecycle: executed truth, 2026-08-29

The four published Fractional actions execute and commit against real ELFs. This
records what runs, what it proves, the shipped defect found on the way, and the
two bounds a Fractional Market now lives inside.

Campaign: `programs/dclutch-claims-sbf/program-test/fractional-atomic/`.

```
cd /Users/ember/dev/dclutch
TOKEN_2022_SO=<a prepared spl_token_2022 v11 .so> \
  programs/dclutch-claims-sbf/program-test/fractional-atomic/run-program-test.sh
```

Ten tests, all green, verified through that script building every ELF from
scratch rather than against binaries already on the machine. Real Claims,
Custody, Registry, Core, Token-2022 and the two-PDA test caller throughout.

## Why this needed a new caller

`fractional_atomic_v3` has shipped for some time and Trading's `hot_v3` already
dispatched its request magic, but nothing had ever run it. The route needs a
caller that signs two PDAs in one `invoke_signed` — the release-scoped Trading
caller-authority at coordinate 0 and the Trading-owned Fractional root at
coordinate 26 (40 in the terminal frame) — and no test caller in the tree could
sign more than one. `test-programs/fractional-atomic-caller/` is that caller. It
forwards the exact production frame and the exact 416-byte request unchanged,
validates the receipt Claims returns, and can refuse afterwards so a late-failure
rollback is provable against real account state.

## The four actions, and what each one proves

Conservation, not acceptance. A test that only asserts "the transaction passed"
would pass against a handler that moved nothing.

| Action | Committed effect |
| --- | --- |
| `Wrap` | Token-2022 supply 0 → 70, the exact denominator multiple of the 7 native Claims locked; every minted shard held by the actor; native Claims move actor → the root's reserve Position and are conserved across the pair |
| `WholeUnwrap` | Exactly inverse, across two separate transactions, so the second reads the first's committed chain state |
| `TerminalZeroBurn` | 44-account frame; every shard burned and locked Claims released; **not one collateral atom leaves the Hoard** and the Custody replay cursor does **not** move |
| `TerminalRedeem` | Real Custody CPI and Token-2022 transfer; every atom the recipient gained left the Hoard; the replay cursor advances exactly once |

`Transfer` is absent from the published set on purpose: it moves an already
issued representation between holders, creates and destroys no native Claims, and
routes `Token2022DirectTransfer`. The campaign pins that from both sides — the
contract routes it away from Claims, and the family caller refuses to carry it in.

Two supporting properties are pinned because they are easy to lose:

- **Rollback** is asserted against the caller's *own* late-failure code
  (`0x10B004`), not merely "the transaction failed". Otherwise the test would
  pass on any earlier refusal, which would mean the commit never happened.
- **The Claims-role Custody replay is created by the real route**, never planted.
  A Claims-role cursor at that namespace is a prestate no route in the tree can
  produce, so a planted one would be evidence of nothing. The campaign runs the
  real `custody_replay_v1` route first, which CPIs Custody to `InitializeReplay`.

## The shipped defect: terminal settlement could not execute

Fixed in `4953bada`. Two shipped requirements contradicted each other:

- `terminal_certificate_v3` requires `core.terminal_receipt` to be `Some`, and
  `CoreState::valid_static` permits a terminal receipt only outside `Phase::Open`;
- the signed delta that settlement performs passed `Phase::Open` into
  `authenticate_core_market_v3`, which compares phases for **exact equality**.

No canonical Core state satisfies both, so every terminal settlement refused with
`SignedDeltaSbfErrorV3::ProductBasis` (`0x5203`) after doing all the work — a
refusal naming the Product graph and the basis, which is not what went wrong and
is why it survived.

**This was not Fractional-specific.** The repository's own Rational campaign was
red before the fix: `a_wallet_held_position_is_paid_from_the_resolved_markets_hoard`
and `a_stale_custody_cursor_refuses_the_second_wallet_payout` both failed, while
their two neighbours passed *because they assert refusals* — passing for the wrong
reason is how a dead route stayed green enough to look alive.

The fix threads a parameter that already existed rather than adding an exemption.
`authenticate_core_market_v3` always took the expected phase; `signed_delta_v3`
was the only layer that refused to pass it on. Now every caller names the phase
its route actually runs in:

| Caller | Phase |
| --- | --- |
| `signed_delta_v3` public submission | `Open` |
| `fractional_atomic_v3` (Wrap, WholeUnwrap) | `Open` |
| `rational_representation_v2` | `Open` |
| `terminal_settlement_v3` | `Terminal` |
| `rational_terminal_v3` | `Terminal` |

Nothing is weakened: phases are still compared for exact equality, and no route
may now execute in a phase it did not name. Controls for the open-market paths,
which must be unchanged: the `affine-batch` and `fractional-signed-delta` real-ELF
campaigns still pass.

### The live cohort carries this defect

The devnet cohort was built from `d3cf6bbf`, which predates the fix. Confirmed
two ways:

- **Static.** All three halves are present at `d3cf6bbf`, and the two decisive
  files are byte-identical (same SHA) to the pre-fix state proven broken.
- **Dynamic.** Running this campaign against the cohort's own `claims.so`: six
  tests pass — wrap, the wrap/unwrap round trip, rollback, the transfer refusal,
  Custody replay creation, the width bound — and all four terminal tests fail at
  `0x5203`.

So on the live cohort a Fractional Market can be founded, wrapped into, traded
and unwrapped, and holders can exit **while it is open**. The trap closes at
resolution: `WholeUnwrap` needs `Phase::Open`, and the two post-resolution exits
are dead. Anyone still holding shards when a market resolves cannot redeem until
a Claims upgrade carries `4953bada`.

## Two bounds a Fractional Market lives inside

**Representation width ≤ 256** is the index space. The `U8` action selector at a
fixed request offset makes 256 the arithmetic bound; the terms codec and
`MAX_COMPOSITION_REPRESENTATION_WIDTH_V3` agree independently. This is why the
shared 258-outcome Claims fixture is permanently out of Fractional range and the
campaign compiles its own geometry at a usable width.

**Supported width 64**, which is narrower and is the one that matters. A terminal
settlement translates every Product result coordinate onto every Claims
representation root, so its cost grows with the width in a way the open-market
actions' does not:

```
  width   8   16   32   48   64    96      98      99
  units 463k 519k 593k 731k 897k 1356k   1393k   exhausted
```

The arithmetic maximum is 98 and the supported width is deliberately **not** 98.
At 98 the margin is 6,672 units of 1,400,000 — under half a percent, inside
build-to-build variation. Width 98 settled against one build of the same
committed source and exhausted the budget against another. A bound that depends
on which machine compiled Claims is not a bound. 64 uses 897,328 units and keeps
roughly a third of the budget in reserve.

This matters because the failure is silent and late. The open-market actions do
no Product evaluation and stay cheap at any width — a wrap at width 256 costs
about 71k — so a 200-outcome Fractional Market would found, accept wraps, and
only fail when holders tried to leave. `FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4` in
`dclutch-fractional-claim-operator` therefore refuses to **publish** a capability
above the supported width, which is the earliest point that trap can be closed.

Refusing a *founding* above that width is a separate decision and belongs to
whoever owns founding. It has not been made.

## Traps worth knowing before touching this

- **Quantity units are not symmetric.** `Wrap` counts native Claims;
  `WholeUnwrap` and the terminal actions count shard atoms. The inverse of
  wrapping 7 is unwrapping 70. Passing 7 refuses `0x5008` with no hint that the
  units differ.
- **Claims does not write the Fractional root.** Trading owns it, so advancing
  the replay revision is the Trading parent's job. Claims' authority over the
  root is to authenticate it and require its signature.
- **Custody must be a different program from Claims**, and refuses aliased
  account frames — the Market's rent beneficiary may not be the payer funding the
  replay cursor. Using one account for both returns `0x6001`, which names the
  shape but not the colliding pair.
- **The paying redemption's Custody caller cannot be guessed.** Coordinate 23 is
  the Claims program on the zero-payout path; the moment the payout is nonzero it
  must be the caller-authority PDA whose seeds commit to the exact
  `CustodyRequestV1`, and so to the payout and the signed-delta packet. Naming it
  requires evaluating the settlement host-side exactly as Claims will. That is
  the seam working as intended: a caller cannot invent an authority for a payout
  it did not compute.
- **A round trip is not a no-op.** Every committed transaction advances the LBV2
  Position replay revision, which is what stops a wrap or unwrap being replayed.
  Asserting raw byte equality across a wrap and its unwrap would be asserting
  that replay protection does not work.
