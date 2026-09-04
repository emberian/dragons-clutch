# The cold machine, 2026-09-03

C-13 asks whether a cold machine can build checked releases, bootstrap, create
representative markets, drive every lifecycle, recover interruptions, and
inspect/export/sign/submit only the intended acts — and it asks that runbooks
contain only commands their campaigns actually replayed. This is that campaign.

**Every command below was run.** Nothing here is transcribed from another
document, and where a command refused, its exact refusal is quoted rather than
paraphrased. What was NOT reached is named twice, at the two points the
campaign stopped: §6 for the first pass, §8 for the second.

The machine is **hbox** (Linux x86-64, 24 cores, co-tenant with codex's HOL
build). Everything the campaign created lives under
`/tank/dclutch-cold-1788448080/`, **14 GB**, left in place. Nothing was written
to hbox's home directory or to any cluster; the only chain used was a local
validator, and it started and died inside that directory.

The comparison host is the laptop (macOS arm64, `--builder local`).

| commit | what it carries | where it is used |
| --- | --- | --- |
| `78176e644` | the tree as found | the first cross-host pair |
| `9e3c4eeff` | + the preflight repair another lane landed mid-campaign | the second cross-host pair |
| `6eb4123cc` | + `run.py` counts the shipped links (§4, defect 1) | third pair; first loopback attempt |
| `fe70f0769` | + the loopback Core projection (§4, defect 6) | **the pair reported in §3** |
| `d532bf6d1` | + the V2-domain succession detector (§4, defect 7) | the loopback run that reached §6's wall |
| `204233776` | + conjunct 6 in the host builder reads the installed profile (§7) | the run that got the succession on chain |
| `34fa44b81` | + the shape assembler keeps the fixture's stated band (§7) | the run that compiled a market |
| `5fa069093` | + the initialize stage admits a superseded V2 (§7) | |
| `48ad76992` | + the rent estimate admits one too (§7) | **the run that founded and opened a market** |

---

## 1. Cold toolchain: 51 seconds, and nothing in the tree said any of it

An empty `CARGO_HOME`, `RUSTUP_HOME` and `HOME` under `/tank`, so the host's own
caches are not inputs.

```sh
export COLD=/tank/dclutch-cold-1788448080
export HOME="$COLD/home" CARGO_HOME="$COLD/toolchain/cargo" RUSTUP_HOME="$COLD/toolchain/rustup"

git clone "$COLD/bundle/<bundle>" "$COLD/src" && git -C "$COLD/src" checkout <commit>   #  2 s

curl -sSfL https://sh.rustup.rs -o "$COLD/toolchain/rustup-init.sh"
sh "$COLD/toolchain/rustup-init.sh" -y --no-modify-path --profile minimal --default-toolchain none
export PATH="$CARGO_HOME/bin:$PATH"
rustup toolchain install 1.97.1 --profile minimal --component clippy --component rustfmt   # 26 s

sh -c "$(curl -sSfL https://release.anza.xyz/v4.0.2/install)" -- v4.0.2 --no-modify-path   # 21 s
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"

cd "$COLD/toolchain" && curl -sSfL -O https://nodejs.org/dist/v26.4.0/node-v26.4.0-linux-x64.tar.xz
tar -xf node-v26.4.0-linux-x64.tar.xz                                                      #  2 s

cd "$COLD/src" && cargo fetch --locked \
    --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml                   #  1.1 s
```

Resulting pins, identical to the laptop's except where noted:
`rustc 1.97.1` host · `cargo-build-sbf 4.0.0` · `platform-tools v1.53` ·
`rustc 1.89.0` SBF · `node v26.4.0` · `npm 11.17.0` ·
`solana-cli 4.0.2 (src:1845f426)` where the laptop reports `src:549805f3`
— both official v4.0.2 builds for their platforms.

The last command is the one nothing documented and whose absence is invisible;
see §4 defect 3.

## 2. The checked release candidate

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
  "$COLD/src/tools/release/checked-release-candidate.sh" \
    --repo "$COLD/src" --work "$COLD/work/candidate-N" --commit <commit> \
    --genesis-cohort \
    --node "$COLD/toolchain/node-v26.4.0-linux-x64/bin/node" \
    --node-archive "$COLD/toolchain/node-v26.4.0-linux-x64.tar.xz" \
    --builder hbox
```

Seven clean runs. `CANDIDATE_EXIT=0` every time, and on hbox
**415, 432, 435, 447, 455 and 460 seconds**; on the laptop
(`--builder local`, the darwin-arm64 Node pin) **389, 435, 526 and 619 seconds**.
Every summary said `sbf_build_freshness=passed`,
**`sbf_build_freshness_links=12`**, `sbf_build_diagnostics_total=0`,
`sbf_build_diagnostics_accepted=false`, `cargo_lock_immutability=passed`,
`cargo_lock_count=70`, `node_version=v26.4.0`,
`spline_product_handoff=passed`, `infrastructure_lineage=genesis`.

A cold machine can build only a **genesis** candidate: a succession needs a
`--predecessor-profile` read off the chain being succeeded.

The eighth run is the one that mattered: run 1 exited **101** having printed
nothing after `SBF build freshness PASS links=12`, 427 seconds in. §4 defect 3.

```sh
python3 tools/cohort14/check-steps.py                     # 19 steps, green
python3 tools/cohort14/check-steps.py tools/cohort15      #  4 steps, green
tools/cohort14/preflight.sh --tests                       # 1 m 13 s, every checkable precondition green
python3 tools/release/plan-sbf-release-batch.py --base <rev> --candidate <rev> --output <path>
```

The last one refused until §4 defect 4 was fixed; it now exits 0 with twelve links.

## 3. The cross-host reproducibility pair C-14 never had

Both hosts, `--genesis-cohort`, commit `fe70f076944ab2a7d5379c9f7b2ee074e0af014b`.

| role | hbox (Linux x86-64) | laptop (macOS arm64) | |
| --- | --- | --- | --- |
| claims | `9c8076bf7a971b5f…` | `bc264881e7dda322…` | differ |
| core | `78da365ad2eed48e…` | `a073b24e9100d300…` | differ |
| custody | `7e4e4745de129249…` | `b4bd159c097d94b9…` | differ |
| dealer-accelerator | `5162692bb47683d9…` | `82c5441fb30ac5b7…` | differ |
| general-accelerator | `b27888978651c770…` | `c8bf00c8ed315b88…` | differ |
| registry | `ac50fd36192d187a…` | `3a6d615de8cf51fb…` | differ |
| rent | `332979111ea18a26…` | `738d847981f60815…` | differ |
| resolution | `cf9a710b94c41c6e…` | `1af6c374e7785e3d…` | differ |
| **series-shadow** | `548dae10fb82be4e…` | `548dae10fb82be4e…` | **IDENTICAL** |
| trading | `8c0f57f882083256…` | `7e581f12c89a56cf…` | differ |

**The control, which is what makes the table mean anything.** Two candidates on
ONE host at two different absolute `--work` roots
(`…/work/candidate-1` and `…/work/candidate-2`, same commit): **all ten ELFs
byte-identical.** The absolute build path is not an input, so the divergence
above is the hosts and nothing else.

**First differing byte, per role.**

| role | first differing byte | in |
| --- | --- | --- |
| claims, core, dealer-accelerator, registry | `0x28` | the ELF header's `e_shoff` — the section header table moved because `.rodata` is 8 bytes longer |
| resolution | `0x121` | `.text` |
| rent | `0x1884` | `.text` |
| trading | `0x501c` | `.text` |
| general-accelerator | `0x9084` | `.text` |
| custody | `0xa66c` | `.text` |
| series-shadow | — | identical |

**The cause is one string, and it is not ours.** Every differing role's
`.rodata` diverges at a `platform-tools` standard-library source path:

```
hbox    /home/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/slice.rs
laptop  /Users/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/slice.rs
```

Anza builds the SBF standard library on its own CI runner, and `core`/`alloc`
panic locations carry that runner's absolute paths into every ELF that can
panic through them. `/Users/` is one byte longer than `/home/`, `.rodata`
rounds up by eight, and every address in `.text`, `.data.rel.ro`, `.dynamic`
and `.rel.dyn` shifts behind it — which is why the difference reads as codegen
and is not. Counted per role: **three** such strings in claims, core, custody,
dealer-accelerator, resolution and trading; **two** in registry and rent;
**one** in general-accelerator; and **zero in series-shadow**, which is exactly
the one that reproduces.

**What the tree's own verifier says.** `successor_campaign_pack.py verify`
passes on both packs, each on its own host — the first time it has ever
verified a genesis pack (§4 defect 5). Its cross-builder projection differs in
`toolchains`, `artifacts` and `release`, and agrees on `source_revision`,
`source_tree_sha256`, both Cargo-lock digests, `ceilings`, `compliance`,
`product_handoff` and `verifiers`. So the frame ceilings and the entire public
Product handoff ARE reproducible across the two hosts; the shipped bytes are
not, and the divergence propagates all the way into the release identities:

```
execution_release_set_id     hbox 457ebc6e977e4bd6…   laptop 804c026ca7f85903…
checked_infrastructure_id    hbox 8350f3fee3fe5e1e…   laptop 7ac4e2d9d6e121b1…
infrastructure_profile_sha256 hbox f27c4c2b11150523…  laptop 003250c486ed2833…
```

**`compare-packs` itself cannot be run across two hosts**, and that is a finding
rather than an accident: it calls `verify_pack` on both packs, and `verify_pack`
binds the Product-handoff smoke's ABSOLUTE run paths, so a pack copied to the
other host refuses with `spline Product handoff differs from its canonical
source fixture`. Verify each pack on its own host and compare the projections;
the verdict above is that comparison, computed with the shipped projection
function.

**The conclusion for C-14.** `supported_builders` is `local`, `persvati`,
`hbox-through-swarm-build`. Its members can reproduce each other only **within
one platform-tools host OS**: hbox and persvati (both linux-x64) can, and a
macOS `local` build can never be byte-identical to either. The pack's
`excluded_nondeterminism` list says it excludes "host OS, kernel, C toolchain,
and libc identity" — the shipped SBF ELFs are not excluded, and they carry the
host OS inside them.

## 4. Runbook and tooling defects, all replayed

| # | where | what it was | how it showed |
| --- | --- | --- | --- |
| 1 | `tools/release/private-validator-lifecycle/run.py` | the literal `13` in three places inside `checked_gate` | `REFUSED: checked release gate does not carry the exact thirteen-link closure`, one second in — the whole loopback lifecycle, dead since `e6b7bf1a` deleted `dclutch-dealer-sbf` on 2026-09-02 |
| 2 | `tools/release/successor_campaign_pack.py` | `len(frames) != 13` | a pack that passed the gate check refused two screens later |
| 3 | `tools/release/checked-release-candidate.sh` and its README | the successor host producer builds `--locked --offline` from a separate workspace lock; a cold cargo cache cannot satisfy it | `CANDIDATE_EXIT=101` with **no stdout at all**, 427 s in; the reason was only in `product-handoff/build.log`: `error: failed to download serde v1.0.228 … --offline was specified`. Four crates, 1.1 s of `cargo fetch`. |
| 4 | `tools/release/plan-sbf-release-batch.py` | `EXPECTED_LINK_COUNT = 13` | `SBF BATCH PLAN REFUSED: program manifest inventory is not exact 13-link set: 12` — the pre-freeze forecast the release runbook tells an operator to run, broken for a day |
| 5 | `tools/release/successor_campaign_pack.py` | `release["predecessor_infrastructure_profile"]` read unconditionally, in `verify_pack` AND again in `reproduction_projection` | `KeyError`, not a refusal, on every **genesis** pack — the only shape a cold machine can build. The supported-builder reproduction had never been run on it. |
| 6 | `tools/local-validator/bootstrap/successor/src/core_bump_projection.rs` | exempts pins whose `deployment_source` is not `observed-programdata-account`, calling that "every loopback and local-validator run"; `local_mutable.rs:149` REQUIRES the opposite spelling | the loopback founding refused on a Core whose live ProgramData hashed to the checked candidate in its own plan. The remedy the refusal names — hand-record the digest — does not scale, because §3 proves those digests are host-dependent. |
| 7 | `tools/local-validator/bootstrap/successor/src/campaign.rs` | the succession detector reads EXISTENCE at the V2 PDA as proof a successor profile was committed | since `c60b25e8` one `InitializeProtocolInfrastructureV1` commits the genesis V2 at that PDA, so `Conflict("V2 profile exists before the successor Registry record is finalized")` fired the instant Initialize landed — 24 transactions into the run |
| 8 | `tools/release/README.md` | seven statements of "thirteen" and the admission criterion `sbf_build_freshness_links=13` | a value no current run can emit: an operator following the README rejects every valid candidate |
| 9 | `tools/release/README.md` | no prerequisites of any kind, and its only worked hbox invocation is a `--predecessor-profile` SUCCESSION | a cold machine cannot run the one example the file shows, and had to reconstruct the toolchain from `docs/board-archive-2026-08-27.md`. `--genesis-cohort` appeared nowhere in the file. |
| 10 | `tools/release/README.md` | the Node pin is given only for linux-x64 | the darwin-arm64 archive name and its `bef4c7e7…` digest are readable only in the script's source, and both flags are required on `--builder local` too |
| 11 | `tools/cohort14/README.md` step 01 | "a SECOND detached worktree at the same commit reproduces all seven ELFs byte-identically" does not say same-host | run across hosts it is guaranteed to fail, for a reason that has nothing to do with the deploy |

Defects 1, 2, 4, 5, 6, 7 are fixed in this series with tests; 8–11 are fixed at
their author. Defect 3's fix is the `cargo fetch` line in §1, now in
`tools/release/README.md` under "A cold machine".

## 5. Inspect, export and sign: the census of what a signature covers

Built cold, `cargo build --release --locked` in the isolated `CARGO_HOME`, 44 s.

```sh
dclutch --help                                   # the read-only surface, six commands
dclutch market decode --base64 <64 zero bytes>   # refuses by length, naming 368 and the 360-byte predecessor
dclutch ticket author --keypair-env DCLUTCH_MAKER_KEY --maker … --market … \
    --collateral-account … --side sell --lifecycle ioc --outcome 3 --generation 7 \
    --nonce 9 --valid-from 11 --valid-through 4294967295 --maximum-fill 100000000 \
    --limit-price 500000 --fee-basis-points 50 --out seller-ticket.json     # 0.55 s
dclutch ticket verify seller-ticket.json
```

`ticket author` printed `signedPreimageBytes: 172` and
`signatureDomain: dclutch/signature/direct-compact-intent-v2`; `ticket verify`
printed the twelve fields and the sentence *"The signature covers every field
above and nothing else."*

**That sentence was tested rather than believed.** Each of the twelve census
fields was altered by exactly one — a decrement, a flipped enum, a neighbouring
address — and re-verified:

```
market  side  lifecycle  outcome  generation  nonce  validFrom  validThrough
maximumFill  limitPrice  feeBasisPoints  collateralAccount
    -> dclutch: REFUSED: this Direct ticket detached signature did not verify
```

Twelve for twelve. The ticket document carries `kind`, `maker`, `signature` and
`intent` and nothing else, so there is no field outside the census to smuggle.
And a flag that would put a key path in the process table is refused at parse:

```
$ dclutch ticket author --keypair <path> …
dclutch: REFUSED: --keypair is refused: pass --keypair-env NAME so the path
never reaches the command line or the process table
```

## 6. What was NOT reached, and exactly where it stops

**No market was founded, so no fill, settlement, census, retirement, or
interruption recovery was replayed on the local validator.** C-13 is not met.

The loopback lifecycle now starts, builds its own host producer, prepares and
authenticates the seven-role mutable genesis, funds its bankroll, and executes
**24 finalized transactions** against a fresh `solana-test-validator` — nine
record bodies published, the Core infrastructure profile initialized, the
Registry upgraded, the successor Registry record finalized — and then the chain
itself refuses:

```
campaign transaction: slot=908 fee=75000 compute_units=241895 initialize Core infrastructure profile
campaign transaction: slot=940 fee=75000 compute_units=2670  upgrade Registry for infrastructure succession
campaign transaction: slot=1036 fee=75000 compute_units=136691 publish record: Finalize DeXB2YK…
Error: Error("the succession was refused before it was built: AlreadySucceeded")
```

This is not a detector bug and not something a cold machine can route around.
`crates/dclutch-operator/src/infrastructure_succession_v1.rs`, conjunct 6 —
"the address, and the vacancy that forbids a second ceremony" — requires the
V2 profile PDA to be **vacant**:

```rust
if state.profile.owner != system_program::ID
    || state.profile.executable
    || !state.profile.data.is_empty()
{
    return Err(Error::AlreadySucceeded);
}
```

Since `c60b25e8`, `InitializeProtocolInfrastructureV1` **fills that exact PDA**
with the genesis V2 body — `runtime::initialize_instruction`'s fifteenth
account, and `model.rs`: *"every cohort this tool plans is born at V2."* The
two are mutually exclusive by construction, so the infrastructure succession
ceremony is unreachable on every cohort this tool plans, and the local-mutable
profile plans one in its genesis fixtures (`REGISTRY_SUCCESSION_BUFFER_LABEL_V1`
is a planted upgrade Buffer, authenticated in `local_mutable.rs:822`).

The decision is a design decision and belongs to whoever owns `c60b25e8`:
either the local-mutable profile stops planning a succession for a born-at-V2
cohort — which `campaign.rs` already has the skip path for, keyed on
`infrastructure_succession.is_none()`, and which cohort-15's runbook already
states as "a cohort born at V2 has NO ceremony" — or the ceremony is deleted as
superseded. It is not resolved here, because both choices change what the
loopback profile proves.

**Resolved in §7, and neither of those two was the answer.** The record below
stands as the campaign that found the wall; §7 is the one that read the design
and got past it.

Everything downstream of a founded market — the fill, the fee settlement, the
ledger census, retirement, the deliberate interruption and its recovery through
`--recover-finalized-founding`, and `dclutch market show` / `capability show`
against a live root — waits on that decision. Nothing about them is claimed
here; §7 founds the market and §8 says where they stop now.

---

## 7. The decision, and the four walls behind the first founding

**Resolved, 2026-09-03, and it was not the architectural contradiction §6 took
it for.** §6 asked whether succession's conjunct 6 should read the installed
profile or whether `c60b25e8` was wrong to fill the V2 PDA. Neither: `c60b25e8`
had ALREADY made conjunct 6 read the installed profile, in the program, and said
so — *"Conjunct 6 becomes ONE SUCCESSION PER DOMAIN rather than one V2 per
domain"*, because raw vacancy *"would now refuse the first real succession of
every cohort that started clean, reinstating P-008 for exactly the cohorts that
never carried the defect."* `programs/dclutch-core-sbf/src/infrastructure_v2.rs`
has implemented that since. `crates/dclutch-operator` RESTATES that conjunct
rather than importing it — a host builder crate cannot link Core — and the
restatement was left behind. The chain would have taken the ceremony; the host
refused it before composing a frame.

The alternative is refused twice over. By the design:
`PROFILE_UPGRADE_RULING_2026_08_31` §6 is *"V2-only in redeployed consumers. No
fallback"*, so a cohort with a vacant V2 stands up complete and can never found
— measured on cohort-9. And by the chain: cohort-14's Core on devnet,
`9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB`, carries at
`5Z4wVRnQiit72FpXAN6zKvosS1mmHTmbYdv4iwv1sQFt` a Core-owned 224-byte `DCLTINF2`
whose two predecessor ids are `6c5e6d81…8f06` and `3ff5e1b5…b938` — the two
genesis sentinels, byte for byte — beside a sealed 144-byte `DCLTINF1` at
`4AyDeALHegigfa7yGDgdR7ZnfKicMghwutGxxoUfekKE`. Read finalized off devnet,
read-only, nothing written. Every cohort standing is born at V2 with its
succession unspent, which is the state option (b) would make unfoundable.

The ruling now carries that amendment inline instead of contradicting the code
it governs.

### The four walls, each replayed

| # | commit | where the loopback died | what it was |
| --- | --- | --- | --- |
| 1 | `d532bf6d1` | administration, 24 tx in | `dclutch-operator` conjunct 6 demanded raw vacancy — §6 |
| 2 | `204233776` | market-input | `founding_band is required to compile a Pyth market` — the shape assembler passed "the caller stated no band" through as "this market has no band", for the one flag of six with no readable value |
| 3 | `34fa44b81` | founding | `--founding-only requires initialize Complete`: `initialize_state` byte-compared the V2 domain against the plan's GENESIS body, which this plan's own ceremony had by then overwritten in place |
| 4 | `5fa069093` | founding | `existing infrastructure profile conflicts with the exact plan coordinate`: the same stale premise in `wallet_arithmetic`'s rent estimate |

Walls 3 and 4 are the same shape as `d532bf6d1`'s and as wall 1's: **a reader
that treats `plan.genesis_infrastructure_profile.body_hex` as "the body at the
V2 domain" when it is only "the body initialization writes."** The remaining
readers were audited: `runtime.rs` verifies that poststate immediately after
initialize, where the genesis body IS the body; `market.rs` routes a plan
carrying a ceremony to `PlannedSuccessor` and reaches the born-at-V2 check only
on the no-succession arm; `terminal_sequence.rs` takes the address, not the
bytes.

### The replay, on hbox, in `/tank/dclutch-cold-1788448080`

Same cold `HOME`/`CARGO_HOME`/`RUSTUP_HOME` as §1, `src7` fetched from a bundle
of each commit, everything through `swarm-build`.

```sh
export COLD=/tank/dclutch-cold-1788448080
export HOME="$COLD/home" CARGO_HOME="$COLD/toolchain/cargo" RUSTUP_HOME="$COLD/toolchain/rustup"
export PATH="$CARGO_HOME/bin:$HOME/.local/share/solana/install/active_release/bin:$PATH"
git -C "$COLD/src7" fetch "$COLD/bundle/<bundle>" 'refs/heads/*:refs/remotes/<name>/*'
git -C "$COLD/src7" checkout <commit>

SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
    "$COLD/src7/tools/release/checked-release-candidate.sh" \
    --repo "$COLD/src7" --work "$COLD/work/candidate-N" --commit <commit> \
    --genesis-cohort \
    --node "$COLD/toolchain/node-v26.4.0-linux-x64/bin/node" \
    --node-archive "$COLD/toolchain/node-v26.4.0-linux-x64.tar.xz" \
    --builder hbox

SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
    python3 "$COLD/src7/tools/release/private-validator-lifecycle/run.py" \
    --repo "$COLD/src7" --release-root "$COLD/work/candidate-N" \
    --validator "$HOME/.local/share/solana/install/active_release/bin/solana-test-validator" \
    --solana  "$HOME/.local/share/solana/install/active_release/bin/solana" \
    --work "$COLD/work/lifecycleN-full-probe" --through full-probe --seeds 1
```

A new candidate per commit, because `run.py` refuses a gate whose
`source_revision` differs from the clean source. Four of them, `CANDIDATE_EXIT=0`
every time: **491, 470, 528 and 454 seconds** — inside §2's 415-460s band.

| lifecycle | commit | exit | elapsed | last stage reached |
| --- | --- | --- | --- | --- |
| 7 | `204233776` | 1 | 275 s | 05-market-input |
| 9 | `34fa44b81` | 1 | 272 s | 06-founding |
| 10 | `5fa069093` | 1 | 283 s | 06-founding |
| 11 | `48ad76992` | 1 | **956 s** | 07-participant-bankroll |

### What the last one actually did

| stage | elapsed | transactions |
| --- | --- | --- |
| 01-prepare-mutable | 2.3 s | — |
| 02-authenticate-mutable | 0.1 s | — |
| 03-local-test-bankroll | 4.0 s | — |
| **04-administration** | **139.6 s** | **38** |
| 05-market-input | 0.3 s | — |
| **06-founding** | **695.0 s** | **189** |
| 07-participant-bankroll | 4.0 s | — |

The two transactions §6 could not reach, and the five that follow them:

```
campaign transaction: slot=909  fee=75000 compute_units=241895 initialize Core infrastructure profile
campaign transaction: slot=1069 fee=80000 compute_units=166767 infrastructure-succession
campaign transaction: slot=1101 fee=75000 compute_units=644826 activate immutable release-set role: Core
campaign transaction: slot=1133 fee=75000 compute_units=729227 activate immutable release-set role: Claims
campaign transaction: slot=1165 fee=75000 compute_units=1203491 activate immutable release-set role: Trading
campaign transaction: slot=1197 fee=75000 compute_units=449656 activate immutable release-set role: Resolution
campaign transaction: slot=1229 fee=75000 compute_units=331178 activate immutable release-set role: Custody
```

and the founding, which had never run on this profile:

```
campaign transaction: slot=7173 fee=75000 compute_units=30660 DCLTGMF3 refuses a substituted Claims request and rolls the whole founding back
campaign stage founding: Open Market QK2Tcr6Z46HH555u8yiUbmL973iZ7DctJGqB9H6WJy3 (23 steps)
```

**A loopback market is founded and Open.** The outer hostile — a substituted
Claims request against the atomic founding — refused and rolled the whole
founding back, on chain, inside that stage.

---

## 8. The fifth wall, which is not this one's

Admissions stop before the first one is composed:

```
Error: the founding's frozen DCLTGMF3 routing table could not be identified on
this chain: 3 frozen lookup tables contain QK2Tcr6Z46HH555u8yiUbmL973iZ7DctJGqB9H6WJy3
and the admission message does not fit a legacy transaction without exactly one
```

`run.py`'s `frozen_founding_routing_table` identifies that table by the only two
facts it believed were on the chain already: the table is frozen, and its
address list contains the market. Its docstring states the premise —
*"The founding creates five routing tables and freezes exactly one"* — and
declines to ask the campaign *"to start writing a sixth thing down."*

**The premise has expired.** `publish_routing_table` now freezes at **eleven**
call sites, and says so in its own docstring: *"Nine call sites took that shape
and two took the frozen one, which is two answers to one question"* — resolved in
favour of freezing all of them, for a good reason (a mutable table is a second
authority over a signed v0 message). Three of those tables contain the market,
so "frozen, and contains the market" no longer names one table, and no
tiebreaker available to `run.py` is a fact rather than a heuristic: last-extended
slot orders them but does not identify them.

So the sixth thing does have to be written down: the founding campaign must
record WHICH frozen table is DCLTGMF3's, and `run.py` must read it instead of
inferring it. That is a change to the founding evidence — the surface the
cohort-15 lane owns — and it is not made here.

Downstream of a first admission the fill, the fee settlement, the ledger census
and retirement (begin → coordinate → finish) are unclaimed, exactly as §6 left
them. The deliberate interruption is further out still and its distance is worth
stating: the chaos matrix is `--through full --seeds 20` only (`run.py` refuses
any other seed count for it) and its seventeen cases run only after all twenty
seeds pass, so no probe-sized run reaches it, and `--recover-finalized-founding`
has no caller in `run.py` at all.

## 9. C-13, stated honestly

**Met.** A cold machine builds checked release candidates (eleven now, four in
this campaign), bootstraps, drives administration end to end — record
publication, initialization, the Registry upgrade, the infrastructure
succession, and all five immutable role activations — compiles a market, and
**founds and opens one**, with its outer hostile refusing and rolling back on
chain. Inspect/export/sign/submit and the twelve-for-twelve signature census are
§5. Every command in this runbook was replayed by the campaign that reports it.

**Not met.** No admission, fill, settlement, census or retirement, blocked at the
routing-table identification above. No interruption was injected or recovered.
`--through full` (twenty seeds plus seventeen chaos cases) has never been run on
any host.
