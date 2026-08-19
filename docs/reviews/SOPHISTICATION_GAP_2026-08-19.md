# Sophistication gap assessment — 2026-08-19

Status: **ASSESSMENT.** An honest inventory of what remains fake, toy-scale,
or absent relative to the project's stated ambition, written against the
sealed baseline at the 100-gate checked manifest. It promotes nothing and
retracts nothing; it exists to aim the next phase. Every claim below cites
the artifact that establishes it.

## 1. The core finding: the joins are the fiction

Individual components carry strong, honestly-labelled evidence. What does
not exist is a join between them that could survive on a public cluster.
Three seams are not merely untested but *structurally impossible* as built:

1. **The source plane.** The mock provider is an account whose body is the
   literal string `MOCK-PROVIDER-V1`, with an invented 77-byte `SRC1` price
   record, and it must be **executable with attacker-chosen bytes** — which
   no public cluster permits (`instructions/source_ingest.rs` mock impls;
   confirmed devnet-impossible by the paces-harness enumeration at
   `9bee35f`). The real Pyth parser lives only in `research/source-profile-v1`
   and has never been called by the runtime. The program has never read a
   genuine oracle account.
2. **Resolution is not publicly reachable.** Resolve requires a
   program-owned evidence buffer that no instruction constructs; every walk
   injects it at genesis (`joined_lifecycle.rs` named-injection list, four
   injections, `896a1cc`).
3. **Nothing has ever matched.** The single settlement slice consumes a
   pre-frozen receipt that no public instruction produces. No order has
   cleared against another order end-to-end in any environment.

## 2. Toy-scale dimensions, and they are load-bearing

| Constant | Value | Location |
| --- | --- | --- |
| `MAX_OUTCOMES` | 16 | `crates/clutch-kernel/src/lib.rs:31` |
| `MAX_PAYOUTS` | 8 | `crates/clutch-kernel/src/lib.rs:32` |
| `MAX_KNOTS` | 16 | `programs/solana-layout/src/lib.rs:157` |
| order slots per page | 16 | order-page codec |
| `SOURCE_ARCHIVE_MAX_RECORDS_V1` | 32, single page | `source_archive.rs:35` |
| terminal model bounds | 4 positions, 4 outcomes | `research/terminal-lifecycle-v2` |

Sixteen order slots is not a book. Thirty-two observations is not a price
history. Sixteen basis functions is a coarse payoff curve for a system whose
distinguishing claim is exact smooth semantics. Multi-page archives are
explicitly refused.

## 3. The compute verdict

Point resolution — the simplest case, a two-bucket window — measures
1,071,197 CU against the 1,400,000 ceiling. Monolithic occupation resolution
misses its admission threshold at best 1,236,364 CU. Direct V2 selection
reaches exactly 1,400,000 and rolls back.

This is not a tuning problem. On-chain re-execution of verification does not
fit, and every axis worth scaling makes it worse. V3's staging buys roughly
one order of magnitude by splitting work across transactions; it does not
buy the next one.

## 4. Absent layers

No fee economics (the only tested settlement is zero-fee; there is no
revenue path in the runtime). No liquidity provision or market making
(`research/liquidity-policy-model` is model-only). No cross-market netting.
No terminal closure (16 of 37 account rows are `UNCLASSIFIED_STOP`; outcome
mints are 82 bytes with no TLV room, so `MintCloseAuthority` is
unrepresentable and they can never close). No upgrade governance — the
authority is whatever key deploys, which `docs/OPEN_QUESTIONS.md:29-32`
records as an undecided P0. No indexer or monitoring. No client capable of
reading the chain: the static client ships `connect-src 'none'` by design.

## 5. Where sophistication should go

**Move 1 — make the crown jewel tradeable.** Exact-rational degree-one
through degree-three spline claims with machine-checked partition of unity
are the project's genuinely novel contribution, and they are currently
inert: markets can be created and resolved but never cleared. Direct V3 is
that unlock and is the correct work in flight.

**Move 2 — answer the compute wall with succinct verification.** The reason
V2 died is that the chain must re-execute a clearing to trust it. The
scalable architecture computes the clearing off-chain and verifies a
succinct certificate on-chain in constant time, which turns book size from a
redesign into a parameter and makes the operatorless story mechanical rather
than aspirational. The project already owns a candidate substrate: the
Lean-authored dual-certificate STARK stack in the adjacent `breadstuffs`
tree (verify-not-find semantics, zero-`sorry`), recorded there as having no
consumer. Joining them is the single highest-leverage architectural move
available, and it is a design phase, not an implementation task — it needs
its own semantic owner, a threat model, and an honest account of what the
FRI/STARK floor does and does not discharge.

**What to stop.** Adding further verified components. The proof surface is
adequate for the current claims; marginal value now lies entirely in joins,
scale, and the economics that would give anyone a reason to participate.
