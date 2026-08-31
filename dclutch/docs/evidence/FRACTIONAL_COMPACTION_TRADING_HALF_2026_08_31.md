# The burn a holder can actually perform, and the layer the Trading half is missing — 2026-08-31

Written by FRACCHECK-2, from FRACCHECK's ten-commit Trading half
(`docs/evidence/FRACTIONAL_CLAIM_CHECK_2026_08_30.md`) and
`docs/design/CLAIM_CHECK_COMPACTION_V1.md` §17.4.

## Result

**No. The sleeping shard holder does not get paid by a stranger's compaction
end to end in the campaign, because there is no compaction to run — and the
reason there is none is one layer deeper than the re-size modelled.**

What the campaign does now prove, end to end and against the audited Token
program, is the leg that was actually in doubt: **the burn is performable.** A
Mint carrying the whole shard profile has its permissioned-burn authority moved
from a program-derived root to a program-derived escrow while the root is still
alive; the old authority is powerless afterwards; and the holder's own signature
plus the escrow's `invoke_signed` burns the shards. §17.4's sound shape is no
longer a design sentence.

What is not proved is everything upstream of that burn, and the sizing of it
moves again. FRACCHECK re-sized the fractional half from eight commits to
fourteen because the redemption route it costed could not exist. The remaining
ten assumed the Trading half was *adding* a route to a composition path that
`RetireCoordinate` already used. **That path does not exist either.**
`RetireCoordinate` is composed at one layer of three, and the two it is missing
are the two a compaction route would need most.

Three commits landed. The re-size below says what the rest costs and why the
number moved.

## What landed

| commit | what |
|---|---|
| `fb125a44` | `read_compacted_shard_mint` in `dclutch-token-svm`: the split-controller sibling, and the disjointness that keeps it from weakening `read_mint` |
| `ee3d1e52` | the hand-off campaign with a derived escrow and a whole-profile Mint, with the behavior profile run over the bytes Token-2022 wrote |
| this one | the amendment and the re-size |

### The split-controller arm, and why it is a disjointness

`read_mint` requires one controller to be the Mint authority *and* the close
authority *and* the burn authority. After compaction's `SetAuthority` the burn
authority is the escrow and the other two are still the root, so the sibling
nominates the burn role separately:

```rust
Token2022BehaviorProfileV2::read_compacted_shard_mint(
    program_id, mint_key, mint_data, expected_controller, expected_burn_authority,
) -> Result<Token2022CompactedShardMintFactsV2>
```

Both entry points now go through one walk, `read_profile_mint`, which authors
the base state, the padding, the account-type byte, the freeze refusal, the
extension set and the optional metadata pair for both. `read_mint` names the
controller twice; the sibling names the burn separately **and refuses
`burn == controller`**.

That refusal is what makes the property a theorem rather than an intention:

> A Mint the live arm admits has `burn == mint authority`. A Mint the compacted
> arm admits has `burn != controller == mint authority`. **No byte string is
> admitted by both, whatever either caller nominates.**

`no_mint_bytes_are_admitted_by_both_arms` asserts it over eight fixtures crossed
with every nomination pair, and *counts* the admissions on each side — two live,
four compacted — so a disjointness that held because nothing was ever admitted
fails instead of passing.

Two decisions worth naming. **A second facts type**, because
`Token2022BehaviorMintFactsV2::controller` promises one key bound identically as
Mint, close and burn authority, and a compacted Mint is precisely where that
promise stops holding; widening the old type would have made its own doc false.
**No `check_compacted_shard_mint`**, because a compacted coordinate's supply is
the durable claim and any holder's redemption lowers it between a request being
built and it landing — pinning it would refuse an honest retirement because
somebody else redeemed first. `read_mint`'s doc says an on-chain caller with an
independent expectation must use `check_mint`; this is a third caller class, and
it is argued in the doc rather than left as a hole.

The profile preimage and its digest are untouched. This is a second reader of
one profile, not a second profile.

### The escrow is derived now, and the profile ran on real bytes

FRACCHECK named two gaps in its own campaign. Both are closed.

**The escrow was a keypair.** It is now built by
`dclutch-claim-check-escrow-signer-test-sbf` from `ClaimCheckEscrowSeedsV1` —
the shipped recipe, bump included — and signed with `invoke_signed`. The
campaign re-derives the same address a second time from the published
`CLAIM_CHECK_ESCROW_SEED_V1` constant and asserts address *and* bump agree, so a
change to the domain or the seed order fails the campaign rather than quietly
testing a different address. Both sides of the hand-off are program-derived,
which is production's shape: an authority that cannot sign for itself hands to
one that can.

**The Mint carried only the burn half.** It now carries the whole shard profile
— root as Mint authority, `MintCloseAuthority` naming the root,
`PermissionedBurn` naming the root, no freeze authority. That is why the signer
program needs a mint action at all: a Mint funded by a convenient keypair
authority is not a shard Mint, and running the profile over one would prove
nothing about the family's Mints.

And the third thing, which neither campaign had. The behavior profile now runs
against the bytes Token-2022 itself wrote, at every stage:

| stage | `read_mint(root)` | `read_compacted_shard_mint(root, escrow)` |
|---|---|---|
| before the hand-off | **admits**, supply 5,000 | refuses `AuthorityMismatch` |
| after `SetAuthority` | refuses `AuthorityMismatch` — and under `escrow` too | **admits**, controller `root`, burn `escrow` |
| after the burn | — | **admits**, supply 4,000 |

`fb125a44` proved the two arms disjoint over fixtures this repository builds.
This proves the swap happens on a real `SetAuthority` — the half a hand-built
fixture can never supply, that the shape the profile refuses is the shape the
chain actually produces.

Also pinned, because without it the campaign would be equally consistent with a
hand-off that had *relaxed* the extension: after the re-point a standard burn is
still `InvalidInstruction` and a permissioned burn naming the escrow without its
signature is still `MissingRequiredSignature`. **The hand-off moves which second
signature a burn needs, and never removes it.**

### One refusal shape, pinned rather than worked around

A failed CPI is not recoverable. The runtime propagates the inner program's
refusal and never consults what the caller returns, so a stranger's hand-off
surfaces Token-2022's own `OwnerMismatch` (`0x4`), not the caller's band. The
campaign asserts the Token code for that reason, and the signer program's
`TokenCpi` arm is documented as unreachable-in-a-transaction-result rather than
deleted. **This is what a validator log will show for the real Claims route
too** — a fractional compaction whose hand-off is refused reports `0x4`, not a
Claims code, and a runbook that promised otherwise would be wrong.

## The finding: `RetireCoordinate` is composed at one layer of three

§17.4 costs the Trading half as "a route in a second ELF", on the precedent that
`fractional_retirement_v3.rs` is permissionless *and* Trading-composed today.
The precedent is weaker than the sentence.

A Claims route reached from Trading's Hot path has to be admitted at **three**
layers. `RetireCoordinate` is present at one:

| layer | where | `RetireCoordinate` |
|---|---|---|
| **execution** | `programs/dclutch-trading-sbf/src/claims_composition_v3.rs` — `route_authority`, `ReceiptKindV3`, `fractional_root_signer`, the receipt verifier | **present** (`route_authority:646`, `fractional_root_signer:869`) |
| **composition decode** | `crates/dclutch-claims-svm/src/composition_v3.rs::decode_selected_with_external`, and `hot_v3.rs::decode_claims_composition_boxed_v3` | **absent** |
| **artifact geometry** | `crates/dclutch-fractional-claim-operator/src/artifacts_v4.rs::action_geometry` / `encode_effect` / `encode_account_profile` | **absent** |

Verified rather than inferred: `FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3` occurs
in `programs/dclutch-trading-sbf/src/` exactly twice, both inside
`claims_composition_v3.rs` — the import and the `route_authority` arm. It occurs
zero times in `composition_v3.rs`. `decode_claims_composition_boxed_v3` builds a
`ClaimsExternalOnceV3` only when the family request carries
`FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2`; every other magic yields `None`, and
`decode_selected_with_external`'s magic chain then falls through to
`ClaimsCompositionErrorV3::Route`. `action_geometry` matches four
`FractionalExposureActionV2` arms and returns `InvalidInput` for anything else,
so no `AccountProfileV2` or `EffectProgram` bundle exists that could name a
retirement frame.

So the only thing that drives `RetireCoordinate` today is a test-only caller
(`test-programs/liability-basis-caller`), exactly as `fractional_atomic_v3`'s
only *production* composition is the exposure magic. **"Trading-composed" is
true of the signature propagation and not yet true of the route selection.** A
fractional compaction route inherits that debt rather than borrowing a solved
problem: it needs its own arm at all three layers, and the two missing ones have
no precedent in this family to copy.

This is not an argument against the design. §17.4's shape is right and the burn
leg is now executed. It is a correction to what the remaining work *is*.

## The re-size, again

FRACCHECK: **fourteen commits, two programs, two cohorts**; four landed, ten
remaining. Three more have landed and the remaining count does not fall by
three, because the composition gap adds work the ten did not contain.

| # | commit | status | note |
|---|---|---|---|
| 1–3b | record, plans, sub-bands, burn campaign | **landed** | FRACCHECK, `2c3b0934`..`afd422d1` |
| 4 | `read_compacted_shard_mint` | **landed** | `fb125a44`; grew a disjointness theorem the re-size did not ask for, and it is the part worth keeping |
| 10a | **new** — the hand-off on a derived escrow and a whole-profile Mint | **landed** | `ee3d1e52`; was inside "the campaign" in the re-size, and is separable because it needs no dClutch route |
| 12 | the design amendment | **landed** | this document and §17.5 |
| 5 | a Trading route that composes fractional compaction | not written | **and now three arms, not one** — see the finding |
| 5b | **new** — the composition-decode arm | not written | `decode_selected_with_external` + `decode_claims_composition_boxed_v3`; no precedent in this family, because retirement never got one either |
| 5c | **new** — the artifact geometry | not written | `action_geometry`, `encode_effect`, `encode_account_profile`, plus a `FractionalFrameKindV3` lock-count row |
| 5d | **new** — the compaction request | **landed** | `6c26d01a`; embeds `TerminalSettlementRequestV3` verbatim, for the reason the native request does. The *receipt* is deliberately not written: a request's field set is determined by what the route must authenticate, which is knowable now; a receipt's is determined by what the route produces, which is not |
| 6 | the Claims compaction route | not written | the largest single piece: a ~48-account frame wrapping the 36-account terminal frame, the `SetAuthority` leg, the record write, the escrow update |
| 7 | the burn-and-pay redemption route | not written | the escrow signs as approver — the leg `ee3d1e52` now proves is sound |
| 8 | `RetireCoordinate`'s compacted arm | not written | reads #4 instead of `check_mint`, admits nonzero supply, skips `execute_mint_close`; its frame gains the record and the escrow, which ripples into 5c |
| 9 | the fractional escrow close | not written | a mixed escrow's outstanding count, as before |
| 10 | the route campaign | not written | smaller than sized now: the Token-level legs are paid |
| 11 | the operator surface | not written | as sized |

**Seventeen commits, of which seven have landed.** Ten remain — the same number
FRACCHECK handed over, after three landed, because the composition gap put three
back. The honest reading is that the *code* remaining is about what was
estimated and the *surface* is wider: three composition arms in two crates
neither FRACCHECK nor FRACR3 had looked at.

If the ten was meant as "one lane", it is wrong by roughly a factor of three.
Commit 6 alone is a lane. The four that landed here are the four that could be
*finished* without the route existing, and choosing them over a partially built
route was deliberate: a half-written 48-account frame would have to be read
before it could be trusted, and nobody would trust it.

## Not verified

- **No dClutch compaction or redemption route was built**, so no route CU was
  measured and §17.3's ~928k projection is still a lower bound on a route that
  does not exist. The only measurements this lane adds are Token-2022's own,
  read off the campaign's own logs and not re-derived: a successful
  `SetAuthority` **1,219 CU** and the stranger's refused one **1,293**;
  `MintToChecked` **1,696**; the refused standard `BurnChecked` **1,407** and the
  unsigned permissioned burn **1,751**. The escrow-signer wrapper's whole
  transaction ran 15,439–16,318 CU, so its own frame — derivation and forwarding
  — is roughly 14k. None of that is a dClutch route's cost and none of it should
  be quoted as one.
- **The escrow PDA lives under the test signer program, not under Claims.**
  `invoke_signed` signs only for the calling program's own addresses, so a
  campaign that wanted a Claims-derived escrow signature would need a Claims
  route that produces one, which is commit 7. What is proved is that this tree's
  escrow seed recipe produces a signature Token-2022 accepts as a burn approver.
- **The Fractional root in the campaign is a stated stand-in**, derived under
  `fraccheck2:root-stand-in:v1` rather than `dclutch:capability-root:v1`. The
  real root's derivation is exercised elsewhere (`fractional-atomic`), and a test
  program deriving the capability-root domain under its own id would produce an
  address that looks like a root and is not one.
- **The Mint is still not one `fractional_atomic_v3` produced.** It carries the
  whole profile and `read_mint` admits it, which is strictly more than the wall
  had; it is still built by the campaign rather than by the Fractional route.
- **Nothing was proved about a mixed escrow**, unchanged from FRACCHECK.
- **`ClaimsCapability` is still stranded**, unchanged from FRACCHECK.
- **No devnet write.** `dclutch-claims-sbf` and `dclutch-trading-sbf` were both
  rebuilt to confirm the `dclutch-token-svm` refactor moves no frame — zero
  diagnostics each, on `tools/gauntlet/run.sh`'s own pattern, as against the new
  signer ELF's zero. The campaign itself loads only Token-2022 and the signer.
- **Both provenance rows were exercised, which was not the plan.** The campaign
  first ran against the canonical Linux artifact (`e2acdfb7…`). A machine restart
  wiped it from `/tmp`, and the campaign *refused* rather than passing on
  whatever else was lying around — the digest gate doing its job, and the one
  time this lane saw it fire. Rebuilding from the pinned crate archive with the
  provenance's own pinned toolchain (`cargo-build-sbf 4.0.0`, `platform-tools
  v1.53`, `rustc 1.89.0`) reproduced `447ca3c6…`, the `macos_arm64_audit` row,
  **bit for bit**. So that row is reproducible on this host from the archive
  alone, and every assertion here holds against both rows rather than one.
  A nearby `spl_token_2022-11.0.0.so` shipped inside `litesvm` hashes to
  neither and was not substituted; its existence is the reason the gate is
  worth having.
- **`tools/sbom` was a named debt here and is now paid.** On this lane's first
  base it was at `STOP` — four manifests `cargo metadata --locked --offline`
  refused — so `SBOM.md` was left alone rather than have their deletion swept
  onto this lane's commit. Main's `c93cdf07` repaired those four; rebasing onto
  it exposed that one of them, `dealer-accelerator/program-test`, had gone stale
  **because of this lane**: its satellite workspace lock transitively covers
  `fractional-atomic`, which now depends on the escrow-signer crate. That is
  `07afa715`'s lesson arriving a second time. The lock is refreshed, all four
  satellites resolve under `--locked`, and `SBOM.md` is regenerated: 54
  manifests, 2,059 rows, **zero unresolvable**, `--verify` `PASS` with no drift.
