# The frontend meets the first live OPEN market — 2026-08-27

The first time `apps/dclutch-web` has been pointed at a dClutch chain that has a
Market on it. Six defects, every one of them fatal to a real read, none of them
visible from a test suite that passed 208 cases.

**Evidence level.** Local-validator execution. Not devnet, not mainnet, and not
an official deployment or frontend. Nothing here signed, submitted, funded, or
published anything, and no address named below is registered anywhere.

## The chain

`tools/gauntlet/run.sh --mode full` at `3b0c58839b78f556bb36ecb706334ed0712bcc09`,
own work root `/private/tmp/dclutch-fd3`. Campaign green: 23 witnesses checked,
0 failed; 100 transactions; the Market reached **Open**.

The campaign's supervisor kills its validator when the run returns
(`ValidatorChild::drop`), so the post-campaign ledger was resumed as a live chain
with `tools/gauntlet/frontend/resume-validator.sh`. **A resumed ledger needs no
launcher and therefore no pinned origin**: it ran on `127.0.0.1:21890`, which
freed the global `20890` slot for another lane's campaign. That is worth knowing
generally — the single global port is only needed for the ~8 minutes a campaign
is actually running.

| Coordinate | Value |
|---|---|
| Open Market | `4fQNy8k7G7bZ9cak6pb2VnigV2F5fbhs7YnYFWQ2LQYH` (phase Open, readiness Consumed, generation 2) |
| Core / Registry / Claims | `2rJGzu…Rqhd8o` / `87syw3…u7mh7e` / `9fAcEn…APBMY6` |
| Claims aggregate | `CUWKSsNsuKsRjSRUJzmERh4gRc6b4XSmWiFBcsVh4Kgj`, 4 claims, supplies `500000000` ×4 |
| Founder | `AVPy5zFJGDSFhcRpYWeqdSjhSbygPPcVH2qL6Cp3ScaA` |
| Founder Position | `129CAcrmwk33Aw8c4b2jCZQaRPVCjgSBVaefkYUyVzGC`, Claims-owned, balances `500000000` ×4 |
| Hoard vault | `8JdqNuFojqCfKXyrF8kdffpgcTok8G1aoWe1ZAnzN8aB`, Token-2022, 500,000,000 atoms |

A second Market (`4fQ…`'s sibling `9k8qkn…`, phase Founding) exists from the
abort lane, which makes the discovery surface's listing a real listing rather
than a single-row special case.

## How this was checked

Screenshots are not evidence — they check a decoder against itself. Three
programs, in `tools/gauntlet/frontend/`:

- **`chain-witness.mjs`** decodes the same finalized accounts a SECOND time from
  byte offsets transcribed out of the first-party Rust, speaking raw JSON-RPC.
  It imports nothing from `apps/`.
- **`drive.mjs`** runs a real headless Chromium: it types into the page's own
  inputs, presses the page's own buttons, waits for the page's own live region,
  and harvests every rendered label/value pair.
- **`compare.mjs`** grades one against the other and exits nonzero on any
  mismatch or any missing field.

## Verification table

**50 of 50 checks MATCH** (`/private/tmp/dclutch-fd3/witness/verification.md`).
Abridged; the file carries every row.

| Surface | Fact | Chain | Browser | Verdict |
|---|---|---|---|---|
| `/markets` | Markets found by the Core scan | 2 | 2 | MATCH |
| `/markets` | phase chip | Open | Open | MATCH |
| `/markets` | per-claim supply vector | `500000000 · 500000000 · 500000000 · 500000000` | same | MATCH |
| `/markets` | exact required backing | `500000000` | `500000000` | MATCH |
| `/markets` | Hoard is refused, not shown | *underivable* | *underivable* | MATCH |
| `/markets/:address` | schema / width | `DCLTCOR2 · version 2` / `352 bytes, exact` | same | MATCH |
| `/markets/:address` | phase / readiness / generation | Open / Consumed / 2 | same | MATCH |
| `/markets/:address` | six content identities | realm, product record, product instance, resolution policy, manifest, release set | all equal | MATCH |
| `/markets/:address` | Claims aggregate / basis / revision / claim count | `CUWKSs…`, `46e2988b…`, 1, 4 | same | MATCH |
| `/markets/:address` | Realm record / mint / token program / adapter | `6EuGAck8…`, `5Dqbx…`, Token-2022, `228c14f9…` | same | MATCH |
| `/markets/:address` | manifest record / content id / 3 entries + kinds + configs | `5wPMUd71…`, `7dc55519…` | all equal | MATCH |
| `/portfolio` | derived aggregate / Position | `CUWKSs…` / `129CAc…` | same | MATCH |
| `/portfolio` | owned claim balances | `500000000` ×4 | same | MATCH |
| `/portfolio` | complete sets mergeable | `500000000` | `500000000` | MATCH |

Screenshots: `/private/tmp/dclutch-fd3/shots-before/` (the refutation) and
`/private/tmp/dclutch-fd3/shots-final/` (after), each with
`markets-enumerated.png`, `markets-discovery.png`, `market-detail.png`,
`market-detail-expanded.png`, `portfolio.png` and the harvested `rendered.json`.
Un-gate: `/private/tmp/dclutch-fd3/ungate/`.

## What reality refuted

### 1. `fetch` could never have worked in a browser (`49516db`)

Every read surface answered
`Refused: Failed to execute 'fetch' on 'Window': Illegal invocation`.
`SolanaRpcClient` stored the ambient `fetch` on the instance and called
`this.fetcher(...)`, giving it the client as receiver. Chromium enforces
`fetch`'s `Window` receiver; Node, jsdom and every injected test double do not.
Every existing case in `lib/rpc.test.ts` passed its own fetcher, so the default
parameter — the only path the product uses — had never been executed.

### 2. The product surfaces decoded a Market no chain writes (`22e785e`, `e8d80e9`)

`getProgramAccounts(core)` returns 352-byte accounts whose magic is
**`DCLTCOR2`**, the Lean-emitted Core state in
`crates/dclutch-market-core-codec/src/generated.rs`. `lib/decoders.ts` knows only
`DCLTCAT1`, so:

- `/markets` said *"3 finalized Core accounts at slot 9326; **0 carry the Market
  header**"* on a chain with two Markets;
- `/markets/:address` refused the Open Market with *"unknown account magic; no
  layout was guessed"*.

Four other web modules (`directHotChain`, `dealerEquityChain`,
`rationalTokenV2`, `rationalRetireReceiptV4`) already decoded `DCLTCOR2`
correctly, so the browser had both representations side by side and the product
surfaces took the dead one.

### 3. The economics are not fields of the Market

A Core V2 root carries identity and lifecycle and **no Hoard figure, no supply
vector, no settlement summary**. Those three were exactly what the discovery
card, the detail page's Economics section and the portfolio's complete-set
arithmetic were built on.

- Per-claim supplies now come from the Claims **LiabilityBasisV2 aggregate**
  (`DCLLBM02`) at `[dclutch:lbv2:market, market]` under Claims, carried as a
  separately provenanced projection. `unread` (no Claims program selected) is
  rendered as unread, **not as an empty vector**.
- The Realm is a **finalized Registry record**, not a Core account; it is
  reacquired at its derived record PDA and re-hashed against the identity the
  Market committed to.
- The **Hoard is reported as not-derivable, with the reason**. Its Custody vault
  is namespaced by the founding's action context — the campaign's own context
  domain is `dclutch/local-campaign/founding-context/v1`, a caller-chosen string,
  not a protocol constant — and the Market root records neither the context nor
  the vault. A reader holding a Market address cannot name that account, so the
  surface says so instead of showing a number nothing authenticates.

### 4. `/portfolio` was confidently wrong about the founder

It derived `[dclutch/position/v1, market, owner]` under **Core** — the Direct
family's Position — and rendered *"No Position exists at the derived address…
this owner has never held a claim in this Market"* about the founder of the
market, who holds 500,000,000 atoms of each of four claims. Those live in a
Claims-owned LiabilityBasisV2 Position at `[dclutch:lbv2:position, aggregate,
owner]`. Without a Claims program the surface now **refuses** rather than
deriving another family's address and reporting its emptiness as an answer.

### 5. Four defects between the un-gate and any real chain (`5129362`)

Detailed in that commit. Briefly: the checked-release decoder rejected semantic
kind `unowned` (the only honest kind for the seven role programs); the activation
plan asked one `getMultipleAccounts` for five whole ELFs, ~5.8 MB base64-framed,
over its own 4 MiB bound; the System Program was required to have an empty body
when a real Agave observation carries `system_program`; and `SYSVAR_OWNER_ID` was
six characters short and not a valid address at all — string-compared only, so
nothing noticed.

## The RL Loader-bytes prediction: **HOLDS, with one structural exception**

`dclutch-release-tool loader-accounts` constructs Loader V3 account bytes offline
from an ELF, and every checked release carries their digests. Nothing had ever
compared them to a runtime. `tools/gauntlet/frontend/loader-prediction.mjs`, run
against the deployed accounts:

```
core        Program      IDENTICAL   36
core        ProgramData  DIFFERS     1007269 vs 1007269   first divergence at byte 13
claims      Program      IDENTICAL   36
claims      ProgramData  IDENTICAL   1073421
trading     Program      IDENTICAL   36
trading     ProgramData  IDENTICAL   1384725
resolution  Program      IDENTICAL   36
resolution  ProgramData  IDENTICAL   527549
custody     Program      IDENTICAL   36
custody     ProgramData  IDENTICAL   355805
registry    Program      IDENTICAL   36
registry    ProgramData  IDENTICAL   220773
rent        Program      IDENTICAL   36
rent        ProgramData  IDENTICAL   152357

13 of 14 constructed Loader accounts are byte-identical to the deployed accounts.
```

**The construction is exact for every genesis-immutable artifact.** The one
exception is Core, and it is not a bug in the construction — it is a semantic gap
nobody had named:

> Loader V3 serializes `ProgramData { slot, upgrade_authority: None }` as
> **thirteen** bytes and writes them over a forty-five byte header. A program
> whose authority has been revoked therefore keeps the **old key sitting inert at
> bytes 13..45 behind a zero tag**.

Measured: deployed Core ProgramData is
`03000000 0000000000000000 00 1653084445a483262ffca938…`, tag `0`, retained
authority `2W9QQUeCfZPD8zDBwGdhhCbDFfN71hgUdQLUHPxKVP3U` — the bootstrap's own
ephemeral authority. The constructed bytes are all-zero there. Same length, one
32-byte window apart. The successor launcher already documents this
("including Loader-retained inactive authority bytes") and the plan pins
`post_revoke_programdata_sha256` separately from the genesis digest.

**Consequence, and it is not mine to resolve:** an offline `loader-accounts`
construction cannot represent a revoked ProgramData, so a checked release over
one can never match the account. Whether a checked release describes the
**artifact** or the **account** is a `dclutch-release-tool` question. Reported,
not patched.

## The un-gate against reality: **CLOSED, three times, for three correct reasons**

Manifests built by `tools/gauntlet/frontend/campaign-checked-release.sh` over the
campaign's own deployed ELFs, program addresses and semantic preimages. The
derived execution release set digest is
`7bf16a5995d23f1ef33d14278fcb0bca1a01a1367c0a34c8bc3470cef07f7faf` — **the exact
set the campaign activated and the exact set the Open Market names**, so the
evidence is bound to this chain and not merely plausible.

| Scenario | Gate | Refusal |
|---|---|---|
| Honest manifests, real chain | **closed** | `2rJGzu…Rqhd8o current Loader account digest differs from its complete checked release` |
| One byte flipped in the trading manifest | **closed** | `trading full checked release does not rebuild the multiprogram evidence` |
| Internally perfect set over a one-byte-altered custody ELF | **closed** | `release-set record 2rxRKfHnaeCJqKfZrJeb39VYNdYZ9DB8kYEX9fsq8jVF is absent at finalized commitment` |

The third is the strongest case: that manifest set is not malformed in any way —
every `create`/`verify`/`inspect` pass agrees with itself. The only thing wrong
with it is that the chain runs a different program, and the refusal comes from
the chain, naming the address it looked for. A gate that only caught malformed
input would have opened on it.

**The first refusal is the honest verdict of this exercise.** The gate stays
closed on truthful evidence, blocked by exactly one artifact for exactly the
reason above. It is not a false refusal by the browser: the digest genuinely
differs. Closing that gap is a release-tool decision, not a frontend one.

## Controls

- Web suite **208 → 231 passing**, 1 skipped; `eslint` clean; `npm run build`
  completes; all six `abi:*:verify` green.
- Every `lib/` test for the three product surfaces is rewritten against
  `fixtures/live-open-market.json` — finalized bytes copied verbatim off the
  validator — with adversarial cases mutating one field of a real account.
- The release fixture stopped lying: Core is revoked in it, the System Program
  carries its real 14-byte body, and every role carries semantic kind `unowned`.
- No unfiltered `-p <crate>` suite was run. No protocol crate,
  `tools/local-validator/**` internal, or other lane's file was touched.

## Left open, deliberately

1. **Two live Market representations.** `dclutch-market-contract`'s `DCLTCAT1`
   and `dclutch-realm-contract`'s Core Realm/Position still exist, still have a
   Rust fixture generator, and `lib/decoders.ts` and `/explorer` still use them.
   Whether that path is superseded is a protocol question for its owner; this
   lane only stopped the product surfaces from taking it.
2. **The Hoard has no chain-derivable address.** Showing a Market's collateral
   principal honestly needs either a Market-root field naming the vault or a
   protocol-fixed context derivation. Today the surface refuses.
3. **The revoked-ProgramData gap** above.
