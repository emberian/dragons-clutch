# The checked release is green, and cohort-12 is staged — 2026-09-02

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized. Nothing in this file has been
deployed: it records a green release gate and a prepared cohort, and it stops
deliberately before the first irreversible step.

Tree root `/Users/ember/dev/dclutch`.

## The gate

`tools/release/checked-release-candidate.sh --genesis-cohort` is **GREEN at
`e39efbb0`**, exit 0, for the first time in this cohort line:

```
sbf_build_freshness=passed          sbf_build_freshness_links=12
sbf_build_diagnostics_total=0       sbf_build_diagnostics_accepted=false
source_revision=e39efbb0b31afeb7a03b10a71b6e2e5d6da0e040
checked Upgrade gate         sha256=5fcf9916cfdeeac09d8a31c790a965e93c895cf66692a24e0e875d87c745435a
successor campaign release pack sha256=f096c225a3df08542c8852be1e08c5b8c2d8daf9585e31c86ad05557921da678
```

All twelve links report zero SBF frame diagnostics, `trading=0` among them —
this lane's own measurement of Direct's `58b077f8`, taken at the commit the
candidate archived rather than read off the commit log.

### Four defects stood between the frame fix and a green gate

None was the frame. Each was found by a full candidate run that built
everything and refused near the end, and each cost about eight minutes to find.

1. **The successor workspace's `Cargo.lock` was stale.** `eb2c6e99` added four
   path dependencies to `dclutch-wallet-terminal-input-operator` without
   recording them. The candidate builds its host tool `--locked`, so it died
   there. Fixed in `9c5e039a`.
2. **`successor_campaign_pack.py` restated the link count as 13.** `e6b7bf1a`
   deleted `dclutch-dealer-sbf` and took the shipped set to twelve; `aa7f8892`
   swept two Rust readers and `0f0ec379` the shell gate, and this was the
   reader nobody reached. The count had **no test at all** — forcing the module
   back to 13 left every one of its tests green. Fixed and tested in
   `fa61b118`, which also repaired a release-freshness test **I broke in
   `0f0ec379`**: it greps the runner's source for a literal that my
   parameterization deleted.
3. **The same file's link LABEL set still named `dclutch-dealer-sbf`.** A
   hand-written set that could never again match the gate it verifies. Both it
   and `ARTIFACT_ROLES` are derived from `SHIPPED_LINKS` now (`92e349e3`).
4. **The root `Cargo.lock` was stale.** `74e044cf` added `dclutch-operator` to
   `dclutch-direct-hot-program-test-support` without it. One line — and the
   candidate byte-compares the lock set of its archived source before and after
   building, so a build that resolves a stale lock rewrites it and the candidate
   refuses. Fixed in `e39efbb0`, the commit the green run then ran at.

Three of the four were one line each. Two were the same defect class as
cohort-11's own blocker (`b2ac8a79`), which is why `90cc4b24` adds a **fast
`locks` CI tier**: `cargo metadata --locked --offline` compiles nothing, so
seventy workspaces check in 28.7 seconds, cheap enough for `cheap` and `all`
rather than only the cut.

## What is staged, and what is deliberately not done

`~/jobs/dclutch-cohort12-20260902/` (mode 700), with a fresh fifteen-key set
(seven program identities, campaign payer, collateral mint and wallet, founder,
and the three founding roles) and the cohort-11 runbook adapted:

| script | change from cohort-11 |
| --- | --- |
| `close-cohort11.sh` | ids **derived from cohort-11's own keypair files**, not transcribed from its evidence doc — that doc is the thing under reconciliation |
| `stage-market.sh` | `--direct-fee-basis-points` **50**, not cohort-11's 30 |
| all others | paths repointed at the new job and the `e39efbb0` worktrees |

The seven role ELFs are **built twice, in two independent detached worktrees at
`e39efbb0`**, on the ordinary release invocation — not the candidate's trading
link, which builds with `--features hot-cu-profile` and is a diagnostic profile
no cohort should run. All seven are byte-identical across the two builds:

| role | bytes | SHA-256 | A == B |
| --- | ---: | --- | --- |
| registry | 234,536 | `ed70f8bda12b77d663126218ad05f36dd77c5bf3100642879cef1441a845afe7` | yes |
| rent | 142,320 | `d46e5f0a64fd7d5e296118c2e7a62a3b67aed2c2ac4420e85069fb8dca632837` | yes |
| custody | 571,432 | `2823c82351638566e295d7f7acc2e559ab61b3ea43750759e84f73bc0f80d567` | yes |
| resolution | 818,368 | `307bc81c604f1a3c52a0dc5ff1b66b094f12faf6be8f8d66b7d337e08c8873e0` | yes |
| claims | 1,366,416 | `268d527e600706b9062921e0a35f0ea2ba13f5bc7790a4351b6b7f0fff5e910f` | yes |
| trading | 2,308,320 | `b0cff55ab0ef162d7e427b8cb894f1468b1804d997ab35c52710df3268a8e3ed` | yes |
| core | 1,187,432 | `9ef7df559565effb780db6b26bf9fd3c89cefb2b86ae5205d37c688d1a5ea58b` | yes |

Registry, rent and custody are byte-identical to cohort-11's; resolution,
claims, trading and core carry everything between `8ae2c9c9` and `e39efbb0`,
Direct's frame repair among them. Trading is 20,808 bytes larger than
cohort-11's, which is what `58b077f8` "and name what it cost" refers to.

**The five checked artifacts the trade needs already exist**, in the green
candidate's work directory as `evidence/{core,claims,trading,resolution,custody}/checked.bin`.
They are the input to `devnet-checked-execution-release-v1`, which produces the
`--checked-execution-release` the Direct producer demands.

### Where this stops, and why

The next step is `close-cohort11.sh`, and it is **irreversible**: cohort-11's
seven program ids can never be reused, and the deployer cannot afford the
redeploy until that close returns its rent. Everything after it must then
complete or devnet has no cohort at all.

That is not a sequence to begin at the end of a long session. The tree already
learned the narrow version of this — cohort-9's runner verifies each role before
the next one starts, because a sequence whose steps spend money must stop at the
first failure rather than after the budget — and this is the same rule one level
up: do not start an irreversible sequence you cannot finish. So the free and
reversible half is done and the spending half is handed over whole.

**Deployer `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` is untouched at
38.738044775 SOL**, the same figure cohort-11's evidence closed on.

## The runbook, in order

Each step from `~/jobs/dclutch-cohort12-20260902/`, Helius key read from
`~/.helius-key` at use time and never echoed:

1. `close-cohort11.sh` — returns ~40 SOL to the deployer. **Irreversible.**
2. `capture-and-prepare.sh <elf-dir> plan.json` — observes the seven live
   ProgramData accounts and prepares the plan. Wait: this observes **cohort-11**,
   which step 1 just closed, so it must run against the NEW deployment — order
   is close, deploy, then capture. Read the script's own header before running.
3. `deploy-all.sh` — seven roles, each verified by dumping the on-chain image
   back before the next starts.
4. `fund-payer.sh` — 2 SOL to the campaign payer, never the deployer as payer.
5. `stage-market.sh` — **check the cuts against live spot first.** Cohort-11's
   `14800,15200` centred a ~$150 SOL; a stale centre founds a market whose
   answer is already known.
6. `ladder.sh execute` then `found.sh`.
7. The two admissions, with `fee_payer` a **different key from
   `position_owner`** — see the population file; the frame requires the owner to
   sign readonly and a fee payer is unconditionally writable.
8. `devnet-checked-execution-release-v1` from the five `checked.bin`, two
   authored tickets, then `devnet-direct-trade-produce-v1`.

## Two things the cohort-12 evidence file must state

**Which address is which.** Cohort-11 has two Core-owned `DCLTCOR3` Market
accounts and its evidence table mixes them; measured off chain,
`ARuPAuyJ…` is phase `0x00` **Founding** and `3rBfDBpa…` is phase `0x01`
**Open** (`STATE_PHASE_OFFSET` is 10). The cohort-12 file must name its Core
Market account and its founding record separately and read both phases back
from the chain rather than from a label.

**The published routing tables.** Since `dc07c73a` every table a founding
publishes is frozen, and `publish_routing_table` refuses unless each reads back
with no authority, non-deactivating, activated before the observation slot, and
byte-exact addresses — so a completed founding is itself the on-chain proof of
the freeze. No lane has founded since that landed, so cohort-12's founding is
the first. List every published table with its address and its readback fields
as observed: **authority `None`, deactivation slot, address count.**

Devnet evidence. Not mainnet evidence.
