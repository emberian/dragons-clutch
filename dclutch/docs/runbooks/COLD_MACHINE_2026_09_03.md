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

**Two clauses of that paragraph are wrong, and §10 measures both.** *"hbox and
persvati (both linux-x64) can"* was an inference from a cause, never a
measurement — §10 runs it, and it is true: ten of ten, byte for byte. And *"the
cause is one string"*, above, is **half the cause**: removing the string
entirely, by installing the linux sysroot into the macOS platform-tools, closes
`.rodata` and the file length exactly and leaves 654 bytes of `.text` moved by a
second channel nothing here suspected. §10 convicts it, and it is what turns
`supported_builders` from a list of hosts into one named builder artifact.

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

---

## 10. The second cause, the named builder artifact, and the cross-host green

**Every command in this section was run**, 2026-09-04, on four substrates: the
laptop (macOS arm64), hbox (Linux x86-64), persvati (Linux x86-64), and a
`linux/amd64` container on the laptop. One commit throughout,
`fe70f076944ab2a7d5379c9f7b2ee074e0af014b` — §3's own commit, so every number
here is comparable to §3's table without re-deriving it.

The source tree was shipped to each host and fingerprinted before anything was
built: the SHA-256 of the sorted `<sha256> <path>` manifest of all 3,247 files is
`32e207323863601bf405857f774e7c4a0ef75a5000498a89bbb0dcf69a428c58` on the
laptop, on hbox and on persvati. The laptop's and the Linux hosts'
`platform-tools` are the same upstream build — `anza-xyz/rust` at `7a87f939…`, `clang`/`LLD` 20.1.7 at
`anza-xyz/llvm-project` `afb02f33…`, `rustc 1.89.0-dev` — differing only in
which host they were compiled *for*.

**The instrument was verified first, and it reproduces §3 exactly.** A plain
`cargo build-sbf --manifest-path programs/dclutch-registry-sbf/Cargo.toml --
--locked` in a scratch directory produced `3a6d615de8cf51fb…` on the laptop and
`ac50fd36192d187a…` on hbox — the two values §3's table reports for `registry`,
from scratch roots on different filesystems from the ones that produced them.

### 10.1 The two strings, convicted to the byte

`registry.so` is 240,432 bytes on hbox and 240,440 on the laptop, and every byte
of that difference traces to two `&'static str` panic locations that Anza's CI
baked into the prebuilt `alloc`:

| host | `.rodata` offset | length | string |
| --- | --- | --- | --- |
| hbox | `0x036fa7` | 83 | `/home/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/slice.rs` |
| hbox | `0x0370c5` | 89 | `/home/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/raw_vec/mod.rs` |
| laptop | `0x036f1b` | 90 | `/Users/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/raw_vec/mod.rs` |
| laptop | `0x036ff2` | 84 | `/Users/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/slice.rs` |

They are the *only* `platform-tools` strings in either ELF. §3's per-role counts
re-derive exactly on both packs — three in claims, core, custody,
dealer-accelerator, resolution and trading; two in registry and rent; one in
general-accelerator; **zero in series-shadow** — and the third string, which §3
counted but never named, is `core/src/iter/traits/iterator.rs`.

Note that the two also appear in the **opposite order**, which is the first hint that a byte-for-byte
rewrite could never have been the remedy: they differ in length, so
canonicalizing them shifts `.rodata` and every address behind it — a relink, not
a rewrite.

Per section:

| section | hbox | laptop | differing bytes in the overlap |
| --- | --- | --- | --- |
| `.text` | 224,752 | 224,752 | 2,215 |
| `.rodata` | 5,352 | 5,360 | 4,560 |
| `.data.rel.ro` | 2,120 | 2,120 | 115 |
| `.dynamic` | 176 | 176 | 3 |
| `.dynsym` / `.dynstr` / `.shstrtab` | — | — | **identical** |
| `.rel.dyn` | 6,560 | 6,560 | 145 |

`.text` is the same *size* and differs in 2,215 bytes: those are `.rodata`
addresses shifted by eight, exactly as §3 said.

### 10.2 Removing the string is not enough, and that is the finding

`--remap-path-prefix` cannot reach a prebuilt `.rlib`, but the *sysroot* can be
replaced. The linux-x64 tarball's `rust/lib/rustlib/sbpf-solana-solana/` holds
only target artifacts, so it was installed into a copy of the macOS
platform-tools and the laptop built `registry.so` against it. Four cells, one
commit, one source tree:

| host | `sbpf-solana-solana` sysroot | `registry.so` |
| --- | --- | --- |
| hbox | linux (stock) | `ac50fd36192d187a…` |
| laptop | macOS (stock) | `3a6d615de8cf51fb…` |
| laptop | **linux (installed)** | `719622c07bdd51c2…` |
| hbox | **macOS (installed)** | `d08680dc6bf968df…` |

Four distinct values, so there are **two** inputs, not one. And the third cell
against hbox's first shows exactly how far the string went:

| section | result |
| --- | --- |
| `.rodata` | **identical**, 5,352 bytes, both strings now `/home/runner/…` in hbox's order |
| `.dynamic`, `.dynsym`, `.dynstr`, `.shstrtab` | **identical** |
| file length, `e_shoff` | **identical** (240,432 / `0x3a8f0`) |
| `.text` | 654 bytes differ, in 55 runs |
| `.data.rel.ro` | 13 bytes differ |
| `.rel.dyn` | 14 bytes differ |

The residue is not noise, and it is not codegen either. The large runs are the
same instruction sequences with a block moved: at `.text+0x232` one side carries
a three-instruction block that the other carries later, and the single-byte runs
around it are branch displacements differing by exactly one instruction. That is
a layout difference.

### 10.3 The second cause: cargo's per-unit metadata hash carries the host triple

With one source tree, one sysroot, one `rustc 1.89.0-dev` and the same
`rust-toolchain.toml` pin, **61 of the 76 dependency `.rlib`s took a different
`-C extra-filename` hash on the laptop than on hbox** — different filenames, not
merely different contents. Of the 15 whose names *did* match, 14 were
byte-identical.

The split is exact and mechanical: cargo computes a unit's `-C metadata` from
its dependency units' metadata, and **build-script and proc-macro units are HOST
units**. Every crate whose closure contains one inherits the builder's host
triple. The 14 identical ones — `bytemuck`, `log`, `lazy_static`,
`sha2-const-stable`, `solana-define-syscall` (all three feature sets),
`solana-atomic-u64`, `solana-native-token`, `solana-sanitize`,
`dclutch-core-contract`, `dclutch-record-contract`,
`dclutch-refusal-registry`, `dclutch-capability-seal-contract` — are exactly the
ones with neither.

**And the shipped ELF is a function of that hash.** The decisive experiment
changes nothing else at all — same host, same sysroot, same source, same flags,
only the value of `__CARGO_DEFAULT_LIB_METADATA`:

```
__CARGO_DEFAULT_LIB_METADATA=AAA   registry.so  c4eb97562efa57d05d9b3a4f37cf22f9…
__CARGO_DEFAULT_LIB_METADATA=BBB   registry.so  4f711ce1bceb87700b6e0277937f6447…
```

So the three remedies the lane was sent to measure all fail, and they fail for
reasons worth writing down once:

- **Post-link canonicalization of the panic strings.** The strings differ in
  length; canonicalizing them is a relink. And it reaches none of cause 2.
- **`-Z build-std` with `--remap-path-prefix`, or `panic_immediate_abort`.**
  Both rebuild the standard library, so both move the frame manifest — and
  neither reaches cause 2 either.
- **Pinning one platform-tools tarball.** Necessary, not sufficient: it *is*
  the third cell above, and it leaves 654 bytes.

**No source change, no flag, and no post-processing can make two different host
triples emit the same SBF bytes.** Only pinning the host triple can, which is
why `supported_builders` had to stop naming hosts and start naming an artifact.

### 10.4 `supported_builders`, defined

**The release is the bytes ONE builder artifact produces, and
`supported_builders` names the hosts that run that artifact.** The artifact is
**platform-tools v1.53 on `Linux/x86_64`**, with the host Rust channel
`rust-toolchain.toml` pins. Its members:

| member | how it runs the artifact |
| --- | --- |
| `hbox-through-swarm-build` | natively, inside `swarm-build` |
| `persvati` | natively |
| `linux-x86_64-container` | any other machine, in a `linux/amd64` container — the laptop's route |

A native macOS build is a **diagnostic** build: real sources, real compilation,
useful for finding a defect, and not a release. `--diagnostic-builder` is how to
make one deliberately; it stamps `release_builder=false` and every pack built
from that candidate refuses.

### 10.5 The cross-host green, ten roles, twice

**persvati against hbox, native, both running the named artifact.** persvati is
a different machine: different kernel (`6.17.0-40-generic` against
`6.11.0-29-generic`), different `$HOME`, different absolute build paths, `cc`
15.2.0 against hbox's. The comparison is against §3's own hbox column
(`candidate-6`, the hbox half of §3's pair), so these are §3's numbers with a
third column added:

| role | hbox | persvati | laptop, `linux/amd64` container | laptop, native (§3) |
| --- | --- | --- | --- | --- |
| claims | `9c8076bf7a971b5f…` | **identical** | **identical** | `bc264881e7dda322…` |
| core | `78da365ad2eed48e…` | **identical** | **identical** | `a073b24e9100d300…` |
| custody | `7e4e4745de129249…` | **identical** | **identical** | `b4bd159c097d94b9…` |
| dealer-accelerator | `5162692bb47683d9…` | **identical** | **identical** | `82c5441fb30ac5b7…` |
| general-accelerator | `b27888978651c770…` | **identical** | **identical** | `c8bf00c8ed315b88…` |
| registry | `ac50fd36192d187a…` | **identical** | **identical** | `3a6d615de8cf51fb…` |
| rent | `332979111ea18a26…` | **identical** | **identical** | `738d847981f60815…` |
| resolution | `cf9a710b94c41c6e…` | **identical** | **identical** | `1af6c374e7785e3d…` |
| series-shadow | `548dae10fb82be4e…` | **identical** | **identical** | identical |
| trading | `8c0f57f882083256…` | **identical** | **identical** | `7e581f12c89a56cf…` |

**Ten of ten, twice.** persvati's column is a full
`checked-release-candidate.sh --genesis-cohort --builder persvati` run,
`CANDIDATE_EXIT=0`, `sbf_build_freshness_links=12`. The container column is a
`debian:bookworm-slim` `linux/amd64` container **on the laptop**, cold
toolchain built by §1's recipe inside it — so the machine that produces §3's
right-hand column also produces its left-hand one, when it runs the named
artifact instead of its own.

### 10.6 The release identities, and the shipped projection function

Every identity §3 reported as diverging is now equal, hbox against persvati:

| field | hbox `candidate-6` | persvati | laptop, native (§3) |
| --- | --- | --- | --- |
| `source_digest` | `4244e275797ccc43…` | **equal** | — |
| `cargo_lock_set_sha256` | `0670b453c77ceb30…` | **equal** | — |
| `multiprogram.execution_release_set_id` | `457ebc6e977e4bd6…` | **equal** | `804c026ca7f85903…` |
| `multiprogram.checked_execution_release_set_id` | `cb2f40463297a8b8…` | **equal** | — |
| `infrastructure.checked_infrastructure_id` | `8350f3fee3fe5e1e…` | **equal** | `7ac4e2d9d6e121b1…` |
| `infrastructure.profile_sha256` | `f27c4c2b11150523…` | **equal** | `003250c486ed2833…` |
| `infrastructure.genesis_profile_v2_sha256` | `7dfc1eb1a689c1dd…` | **equal** | — |

And the whole comparison, computed the way §3 says it must be — verify each
pack on its own host, then compare the shipped projection function's output:

```
hbox      projection_sha256 0e50ca5658ec2d07a250ebeb8ea1dae7d61f415e63cb2c59726ad0f155863db2
persvati  projection_sha256 0e50ca5658ec2d07a250ebeb8ea1dae7d61f415e63cb2c59726ad0f155863db2
```

Section for section: `toolchains` `867e4e0f…`, `artifacts` `137f36f3…`,
`release` `e18170f0…`, `ceilings` `5828269f…` with **12 frame rows** — all four
equal. §3's cross-builder projection differed in `toolchains`, `artifacts` and
`release`; between two hosts running the named artifact it differs in nothing.

Two notes on how that was computed, because both are findings:

- `verify_pack` binds the Product-handoff smoke's ABSOLUTE run paths, so a pack
  cannot be verified on the other host — §3's finding, unchanged, and the reason
  `compare-packs` is still not a cross-host command.
- `reproduction_projection` at `fe70f076` **raises `KeyError` on a genesis
  pack**, which is §4 defect 5's shape surviving in the one function §3 said it
  had used. HEAD's copy carries the `release.get(...)` fix, so the projections
  above were computed with HEAD's function over each pack verified by its own.
  This lane's change adds the same guard for the toolchain keys: a pack that
  predates a projected field now **refuses** rather than raising.

### 10.7 What changed, and what is owed

`supported_builders` is now `hbox-through-swarm-build`, `persvati`,
`linux-x86_64-container`, beside a `release_builder_artifact` record naming
platform-tools and `Linux/x86_64`. `checked-release-candidate.sh` refuses a
non-artifact host after every argument check and before any source or build
work, and `--diagnostic-builder` is the stated way past it; it stamps
`release_builder=false`, which `successor_campaign_pack.py` refuses.
`reproduction_projection` carries `release_builder_artifact`, so two packs are
compared only when they name the same producer. `tools/cohort`'s
`redeploy-named-builder` row states the artifact from cohort 16 on, replacing a
verifier whose "second detached worktree" clause named the build-path control
and called it the reproduction.

**Owed, and not this lane's:**

- `tools/release/private-validator-lifecycle/test_preflight.py` is 15 failures
  and 3 errors at HEAD, on `runner TERMINAL_SESSION_SCHEMA differs from semantic
  owner tools/local-validator/bootstrap/successor/src/terminal_sequence.rs`. It
  is the only red row in `tools/ci/run.sh release` after this lane's nine
  additions, and it belongs to whoever owns that schema.
- `tools/cohort`'s `found-general-family` row has no README heading, so
  `check-steps.py --cohort 16` refuses for a row this lane did not write.
- The two release-path breakages in §10.8: one fixed here, one owed by SERIES,
  and together they mean **no checked release candidate completes at HEAD**.
- The frame ratchet is red for four commits from ECONOMICS, SERIES and RECOVERY.
  This lane touches no crate compiled into an SBF link and owes no rows.

**Not done here:** the `linux-x86_64-container` recipe is a bare
`debian:bookworm-slim` plus §1's cold toolchain, run by hand. It is not a
committed image, not pinned by digest, and not wired into any runner — so the
laptop's route to the supported set is measured and reproducible but not yet
one command.

### 10.8 The same result at a second commit, and the two things that stop a candidate at HEAD

Everything above is one commit, so it was run again at another: `7d2f91e5f`,
this lane's own HEAD, 30 commits later with real program changes from four other
lanes in between. hbox and persvati, both `--genesis-cohort`, the release
runner's own per-package target layout on each:

| role | `fe70f076` | `7d2f91e5f` | hbox vs persvati at `7d2f91e5f` |
| --- | --- | --- | --- |
| claims | `9c8076bf7a971b5f…` | `5449a2742d14a476…` | **identical** |
| core | `78da365ad2eed48e…` | `5f73bf5f03a873a5…` | **identical** |
| custody | `7e4e4745de129249…` | `5c1f1198dcbb3326…` | **identical** |
| dealer-accelerator | `5162692bb47683d9…` | `5ded31423ec9dae4…` | **identical** |
| general-accelerator | `b27888978651c770…` | unchanged | **identical** |
| registry | `ac50fd36192d187a…` | `6d4792d5a2c16359…` | **identical** |
| rent | `332979111ea18a26…` | unchanged | **identical** |
| resolution | `cf9a710b94c41c6e…` | `d37b13bf5984d54c…` | **identical** |
| series-shadow | `548dae10fb82be4e…` | unchanged | **identical** |
| trading | `8c0f57f882083256…` | `09692b8b70eb0d80…` | **identical** |

Seven roles moved between the two commits and three did not, and every one of
the ten agrees across the two hosts. The result is not a property of one commit.

**Neither run finished, and the two reasons are both cross-lane debts on the
release path that this campaign found by trying to use it.**

1. **`tools/local-validator/bootstrap/successor/Cargo.lock` did not carry a
   dependency edge.** `06008f46b` (ECONOMICS) made `dclutch-direct-codec`
   depend on `dclutch-protocol-parameters-contract`; that workspace is the
   source-pinned **Product producer**, built `--locked --offline` at
   `checked-release-candidate.sh:938`, so every candidate at HEAD refused there
   — §4 defect 3's exact shape, a nonzero exit whose reason is only in
   `product-handoff/build.log`. **Fixed in `7d2f91e5f`**, eight cargo-generated
   lines. **Twenty-five other workspaces are red the same way and are not**:
   `tools/dclutch-cli`, `tools/devnet-scenarios`, `tools/ticket-board`,
   `tools/direct-translation-validator`, `crates/dclutch-svm-harness`, three
   `tools/gauntlet` workspaces and seventeen `programs/*/program-test`
   workspaces. The root workspace is green, so the SBF links were never at risk.
   One command per workspace is the whole discriminator:
   `cargo metadata --locked --offline`.

2. **`SeriesCurrentReleaseInputV5` gained a field and one consumer was not
   swept.** `97ce7a748` (SERIES) added `template_occurrence_count`;
   `tools/local-validator/bootstrap/successor/src/series_terminal_campaign.rs:823`
   still constructs the struct without it, so the Product producer fails to
   compile at committed HEAD with `E0063`. **Not fixed here, deliberately**: the
   field's own documentation calls it release geometry — a one-occurrence
   Template pins 128 bytes and declares no proof range, an `n > 1` Template pins
   `128 + 32 * ceil(log2 n)` and declares one — and the campaign struct carries
   no occurrence count to read it from. The operator's test passes `1`; writing
   `1` here would be choosing a release geometry by guess. The question that
   belongs to SERIES is: **what is this campaign's Template occurrence count,
   and which of its inputs already knows it?**

So **no checked release candidate can be completed at HEAD today**, and neither
half of that is this lane's. What the two runs do prove is the part upstream of
them: twelve fresh SBF links, `sbf_build_freshness=passed`, and the ten shipped
ELFs identical across two hosts running the named builder artifact.

**One more thing the pack tool proved about itself.** The new
`successor_campaign_pack.py` `emit` was run against the real `fe70f076`
persvati candidate and wrote the intended block —
`supported_builders` = `hbox-through-swarm-build`, `persvati`,
`linux-x86_64-container`, beside
`release_builder_artifact = {platform_tools 1.53, Linux, x86_64}`. `verify` on
that pack then **refused**: *"executing campaign pack verifier differs from the
pack's exact source revision"*. That is correct and is worth writing down — the
verifier binds itself to the candidate's own source copy, so a pack cannot be
verified by a tool the candidate did not contain. It also means a full
emit-and-verify of this change needs a candidate whose source IS this commit,
which is what item 2 above blocks.
