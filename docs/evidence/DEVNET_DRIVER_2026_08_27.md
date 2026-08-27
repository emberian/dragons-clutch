# The devnet campaign driver — W3 closed, and the rail that replaced the rail

**Date: 2026-08-27. Lane: DRIVER. Status: the driver exists, reaches devnet
read-only, and is proved against a local validator. Every number in this file is
measured unless it says otherwise.**

**The one-line verdict: the road from a deployed program set to a living market
now has a vehicle, and it can be driven at devnet without being able to reach
mainnet.** `docs/evidence/DEVNET_SMOKE_0.md` §2 W3 recorded that no driver in
the tree could speak to a non-loopback cluster and that the charter's "per-role
`--keypair` flows" did not exist. Both are now false. What is still open is
named in [§7](#7-what-the-durable-deploy-still-needs), and the largest item is
not this lane's: it is the founding stage's market input, which is a wiring
question with real principal behind it and is deliberately left refusing rather
than half-wired.

Charter: SMOKE-0 §6 item 2 (the devnet driver), against
`docs/decisions/0012-devnet-iteration-substrate.md` as ruled. Item 3 (the devnet
`PythReleaseV1` row) was minted by SMOKE-0 itself at `11f249ff` while this lane
was starting; this lane consumes it, re-verifies it live, and reports one defect
in it ([§6](#6-the-pyth-row-consumed-re-verified-and-one-defect)).

---

## 1. The rail, and why replacing it is not weakening it

`runtime::rpc_origin` refused any RPC origin that was not literal `127.0.0.1`.
That single rule was doing two different jobs:

1. **The supervisor must talk to the validator it started.** The launcher binds
   `127.0.0.1` and nothing else, so a spec naming `localhost` or `[::1]` passes a
   loopback test and then fails to reach the process the campaign owns.
2. **Accidental mainnet must be impossible.** A campaign signs revocations,
   funds accounts and moves principal; one mistyped URL is the whole disaster.

Job 1 is real and unchanged. Job 2 was a *side effect* of job 1, and a devnet
driver has to give up job 1 without giving up job 2. So job 2 stops being a side
effect and becomes the explicit rule that `cluster.rs` now states:

| origin | admitted? | ceremony |
|---|---|---|
| `http://127.0.0.1:PORT/`, credential-free, port in the launcher's derivable range | yes | none — byte-for-byte the old rule |
| any other host | only with `--i-mean-devnet <DEVNET_GENESIS_HASH>` | the flag's **value is a cluster identity**, not a boolean |
| anything mainnet-shaped | **never** | no flag, no environment variable, no URL spelling |

**Mainnet is refused at three independent points**, and the third is the one
that actually holds:

1. **The acknowledgment cannot name it.** `--i-mean-devnet` accepts exactly
   devnet's genesis hash; mainnet-beta's own hash gets its own refusal.
2. **The host shape is refused statically**, before a byte leaves the machine —
   a label-wise match on `mainnet`, so `api.mainnet-beta.solana.com`,
   `solana-mainnet.g.alchemy.…` and `mainnet.helius-rpc.…` all die locally. This
   is deliberately a *heuristic* and deliberately not the gate.
3. **The cluster's own `getGenesisHash` is checked at connect**, before any
   transaction is constructed — and mainnet's hash is refused **even on a
   loopback origin**, because a loopback port can be an SSH tunnel and "I was
   sure it was local" is the shape every one of these accidents has.

Point 3 is why the URL rules can afford to be heuristics: no spelling of a URL
talks its way past a hash the chain reports about itself.

The flag's value being the genesis hash rather than `true` buys one specific
thing: **a command line copied to another cluster stops being true.** A `--yes`
survives a copy-paste; a 44-character cluster identity does not.

### 1.1 The three other questions an origin answers

Origin was never only "may I connect". `cluster.rs` is now the single owner of
three answers that were previously re-derived at each site:

| question | loopback | acknowledged devnet | why |
|---|---|---|---|
| may launch a validator | yes | **no** | the driver launches nothing; `rpc_origin` refuses a non-launching origin at the supervisor's entry |
| may airdrop | yes | **no** | devnet's faucet is rate-limited far below a campaign's needs, so a driver that begged for lamports would fail *inside* a ladder instead of at preflight with an exact shortfall |
| may derive keys from a seed | yes | **no** | `seed.rs`'s footgun, unchanged; the answer moved to `cluster::seeded_keys_admissible` so widening the origin allowlist can never widen it by accident |

The seed set is deliberately *wider* than the launch set — `localhost` and
`[::1]` cannot carry a key off this host either — and
`cluster::tests::no_acknowledged_origin_can_ever_admit_a_seed` proves the
property that matters: an acknowledged origin is never in the seed set, because
the acknowledgment is refused on a loopback URL outright.

---

## 2. The command surface

```text
dclutch-local-successor-bootstrap campaign
    --rpc-url URL
    [--i-mean-devnet GENESIS_HASH]
    --plan ABSOLUTE_JSON
    --keypair-ROLE ABSOLUTE_KEYPAIR_JSON ...
    [--evidence ABSOLUTE_JSON]
    [--through STAGE]
    [--execute]
```

Two flags are deliberately **absent**. There is no `--keypair-seed`: a
reproducible private key on a public cluster is exactly the footgun `seed.rs`
documents. And there is no `--force`: every refusal this driver raises is a
statement about the chain, and the fix is to change the chain or the plan, never
to tell the tool to stop noticing.

### 2.1 Per-role `--keypair`, and the one property an operator depends on

`--keypair-<role>` takes an ordinary Solana CLI keypair file. The nine roles are
the campaign's own (`seed.rs`'s `role` module), so the flag names the same
things the key forge does rather than inventing a second vocabulary.

**Index 0 of each role is that file's own key.** The address
`solana address -k core-upgrade-authority.json` prints is the address the
campaign pays fees from, with nothing derived in between — because funding
happens outside this tool and an operator cannot fund an address they cannot
print. Higher indices are `SHA-256(DOMAIN || 0 || file-secret || 0 || role || 0
|| n)`, so a role that needs a second key never needs a second file: demanding
one would make the operator's obligation depend on a control-flow detail they
cannot see, and a missing file would surface halfway through a founding ladder.

The domain is distinct from `--keypair-seed`'s, so the same 32 bytes used as a
lab seed and as a persisted file can never produce the same key
(`seed::tests::the_two_keyed_origins_are_domain_separated`).

A keypair file is read as 64 bytes and **the declared public key is re-derived
from the secret half and compared**. A damaged or hand-edited file is a refusal
that says *do not fund the address it prints*, rather than a signature the
cluster rejects later for reasons that look like something else.

Evidence honesty: a driver run reports `keypair_derivation:
"persisted-per-role"` and `private_key_persisted: true`. The supervisor's
constant `false` is not inheritable — a key that outlives the process is exactly
what these are.

### 2.2 Reads before writes, enforced rather than intended

`--execute` is opt-in. Without it the connection is opened `ReadsOnly`, and a
**method allowlist refuses any non-read at `Rpc::call`** — the single point every
request in this tool passes through. This is the same shape
`tools/release/devnet-observe.sh` uses for the same reason: a preflight that
*cannot* write is worth more than one that intends not to. The allowlist is a
literal list, not a `starts_with("get")` rule, because `getFeeForMessage` is a
read and `requestAirdrop` is not and neither is decidable from the prefix.

### 2.3 Pacing, and the per-IP lesson

SMOKE-0 friction 1 measured one busy writer starving *every other request from
the same IP*, a 1-per-20-second account poll included. So:

- every call on a devnet connection waits out a **250 ms minimum interval**
  (measured-profile, from SMOKE-0's observations — four calls a second keeps the
  confirmation loop responsive while staying far below the rate one
  `write-buffer` was measured to consume). Loopback is unpaced: there is no
  shared budget to starve and the wait would only slow a local campaign down.
- the driver is a **single sequential writer by construction**. It never holds
  two write buffers open and never fans out.
- transaction confirmation became **deadline-based rather than iteration-count
  based**. The old `for _ in 0..600` at 100 ms was equivalent while every
  connection polled an unpaced loopback validator; on a paced connection each
  iteration also waits out the call interval, so a fixed count silently becomes a
  different — and unstated — amount of patience. Loopback keeps its 60 seconds
  exactly; devnet gets the 300 seconds its profile names.

---

## 3. Resumability: the chain is the state file

Devnet dies mid-ladder — SMOKE-0 measured exactly that and resumed into the same
buffer. So **every stage detects its own completion by reading the chain, and
there is no local state file.** A state file can disagree with the chain; the
chain cannot disagree with itself. Re-running the driver after any failure is
always safe and always the right move.

Each detector returns one of four states, and `partial` is named rather than
collapsed into either neighbour, because a half-published record set is exactly
what a devnet outage leaves behind:

| state | meaning | what a resumed run does |
|---|---|---|
| `absent` | nothing of this stage exists | execute it |
| `partial` | some of it exists | execute the rest |
| `complete` | the stage's own poststate verifier passes | skip it |
| `conflict` | it exists and is **wrong** | **stop**; never write over it |

The detector is deliberately the *same* poststate check the supervisor runs after
executing the stage. A detector agreeing with a weaker condition than the
verifier would let a resumed run skip work that never completed.

| stage | detector reads | writes |
|---|---|---|
| `substrate` | each role's `ProgramData`: observed deployment slot and upgrade authority against the plan's pin | **never** |
| `publication` | the nine infrastructure record bodies at their derived coordinates, byte-compared | Registry `Begin → Append → Finalize` |
| `initialize` | Core's infrastructure profile body | one `InitializeProtocolInfrastructure` |
| `activation` | the release activation cache, through `verify_activation` | the role activation ladder |
| `founding` | — | refuses; see §7 |

---

## 4. Decision 0012: the driver has no revoke stage, and that is the point

This lane began with a revoke stage and a `--i-accept-irreversible` gate for it.
Ember's ruling (`docs/decisions/0012-devnet-iteration-substrate.md`, `75adf653`)
retired both: the durable devnet substrate is **mutable and iterated**, and the
Loader's slot write is the invariant immutability was buying.

What the driver owes that decision is the other half of it. `substrate` reads
each role's **observed deployment slot** and compares it to the slot its release
binds — which under 0012 *is* the whole invariant, because every `Upgrade` writes
the current slot and there is no path to different bytes at one program id that
does not go through `Upgrade`. So a moved slot is reported as what it is:

> **SLOT DRIFT (decision 0012 fail-closed)**: … The substrate was upgraded after
> this plan observed it. Every market founded on the old generation is refusing
> right now, which is the designed behaviour. Re-mint this plan's release bodies
> from the CURRENT observed ProgramData before publishing anything.

Not a deploy error. The fail-closed condition, named, with the next action.

The stage also never writes: **this driver does not deploy programs and has no
code path that could.** Deployment is `solana program deploy`'s job, it is the
act that parks ~31.7 SOL, and it is not something a campaign driver should be
able to do as a side effect of a flag.

### 4.1 The driver hit decision 0012's wall executably, from the other side

The local proof ([§8.3](#83-the-local-proof-the-driver-does-its-own-writes))
ran the driver's write path against a validator whose Core still holds its
upgrade authority — which is exactly the mutable substrate 0012 calls for. It
published all nine records and initialized the profile, and then **activation
refused**:

```text
Program 87syw3eBN…  failed: custom program error: 0x1004
                     = RegistrySbfError::Release
                     (programs/dclutch-registry-sbf/src/lib.rs:80)
consumed 501,832 of 1,399,850 compute units
```

`immutable_release_elf_digest_v1` refuses any observed upgrade authority, so a
release set cannot be activated over a still-authoritative Core. This is SMOKE-0
wall W1 reached **from the driver's side, as a live refusal on a real
transaction**, rather than by reading the code — and it is the precise condition
`docs/decisions/0012-devnet-iteration-substrate.md` §4 exists to remove (the
digest-reuse arm extended to `ExactAuthority` when the observed slot equals the
release's bound slot).

**So the driver's stage ladder is correct for 0012's world, and 0012 is not
implemented yet.** The driver has no revoke stage because the ruling retired it;
until PIN-0012's admission lands, `activation` is the ladder's terminal wall on a
mutable deployment. That is stated here as a dependency rather than worked around
— re-adding a revoke stage would be implementing the decision ember overruled.

The refusal is also, incidentally, a clean demonstration that the driver's
failures are *legible*: it names the role, the code, and the compute consumed,
because the refusal came back through the same `send` path the supervisor uses.

---

## 5. Transport: where SMOKE-0's 100× actually applies

SMOKE-0 §3.1 measured TPU submission at ~100× `--use-rpc` for **buffer writes**
(Trading's 1.32 MB in 23 s, single pass, zero retries, against ~350 B/s and
`Max retries exceeded`). §6.4 says the rest in its own words: *"the founding
ladder + life are RPC-shaped end to end."*

So the honest reading of the measurement is that the 100× belongs to the
~1,310-write buffer ladder, which is the **CLI's**, not this driver's.
Re-implementing a QUIC TPU client here to submit the founding's ~116 *sequential*
transactions — each of which must be confirmed before the next is built from it —
would buy nothing the measurement supports and would put a second transaction
transport into a tool that has one.

What the driver does instead is **state the policy and emit the ladder**:
`campaign::deploy_ladder` prints the exact `solana program deploy` commands with
TPU as the default, `--use-rpc` named as the fallback for a machine whose TPU
egress is blocked, and friction 1's consequence spelled out — *run one at a
time; nothing else may share this machine's IP while a buffer is writing.* The
transport policy is tested
(`campaign::tests::the_deploy_ladder_defaults_to_tpu_and_never_executes`) and
printed in every report. It is not silently assumed, and it is not a
reimplementation nobody measured.

---

## 6. The Pyth row: consumed, re-verified, and one defect

Item 3 of the charter was minted by SMOKE-0 at `11f249ff` while this lane was
orienting. This lane consumed it rather than re-minting it, and re-verified it
two independent ways.

### 6.1 Live re-verification through the driver, 8/8

The preflight authenticates the committed row against the real devnet accounts —
the same joins `provider_instruction_v3::authenticate_pyth_release` makes on
chain, run *before* a market is founded against them rather than discovered as a
refusal at resolution:

| fact | result |
|---|---|
| receiver program `rec2HH…` | executable, owned by `BPFLoaderUpgradeab1e…` |
| receiver deployment slot | **observed 460,336,311 — row binds 460,336,311** |
| receiver upgrade authority | `upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr` (disclosed; the row does not bind it) |
| router program `HDw2E7…` | executable, owned by `BPFLoaderUpgradeab1e…` |
| router deployment slot | **observed 460,336,290 — row binds 460,336,290** |
| router upgrade authority | `upg8KLAL…` |
| receiver `Config` digest | **observed `23a7a19c…a37a` — row binds the same** |
| receiver `Config` owner | `rec2HH…`, the receiver itself |

Eight of eight, against the live cluster, in a 9.2-second read-only run.

### 6.2 The defect: `receiver_abi_id` is not the receiver ELF's digest

Independently, by base58-decoding the runbook's own spellings and re-hashing the
committed fixture accounts, seventeen of the row's eighteen fields verify
exactly. One does not:

```text
committed   c507955864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af604
sha256(fixtures/pyth/local-upgraded-2026-08-22/receiver.so)
            c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64
```

The committed constant is that digest with the hex nibble at index 7 dropped and
the tail repacked. The correct value is also what `11f249ff`'s own commit message
cites as dual-confirmed, what `docs/design/DEVNET_DEMO_DEPLOY.md` §5.2
tabulates, and what the fixture `PROVENANCE.md` records.

It was **inherited, not introduced**: the pre-inversion `synthetic_fixture.rs`
literal carried it, and
`crates/dclutch-svm-harness/tests/support/pyth_provider.rs:376` repeats it with
the resulting record digest pinned at `:403` — so code and tests agree with each
other and disagree with the ELF, and no green suite could have caught it.

**Why it still matters though nothing enforces it.**
`authenticate_provider_program` binds the five keys and the two deployment slots,
never an ELF hash, so `receiver_abi_id` is pure committed evidence — which makes
this silent forever, and which is exactly why it wants fixing before the
440-byte preimage is published as a Registry record. A release row is the most
durable place this project writes a measured fact. Correct bytes:

```text
c5 07 95 59 86 4f c3 4d bd 5f e8 7b 4a a9 fb a3
a1 ed 22 69 03 63 ec 49 04 49 e8 66 0e 73 af 64
```

Fixing it moves `pyth_provider.rs:403`'s pinned record digest, which is the gate
doing its job. Raised to SMOKE-0 on the wave board twice (before and after the
commit); left with its owner rather than edited across a lane boundary.

---

## 7. What the durable deploy still needs

1. **The founding stage's market input.** `market::execute_found_market` is
   already origin-agnostic — it takes only `(&mut Rpc, &plan, &market input,
   &authority, &forge, &mut transactions)` and never asked where the chain came
   from. What it needs is the run spec's `market` block reaching the driver's
   entry, and the driver takes a **plan**, not a spec. This is a wiring question
   with real principal behind it, so it is left **refusing with that exact
   sentence** rather than half-wired: a founding that fails midway costs money.
2. **The life's stages.** The journey runner produces the N=4 life and the L1–L7
   conservation ledger, and it is loopback-bound *by design* — its founder key is
   ephemeral and never persisted, so the life exists only in-process against the
   validator the runner itself launches. The persisted-per-role forge in this
   lane is the missing half of that decision; wiring it into the journey is the
   next lane, and it is the "persistent-founder decision it was designed to
   refuse" SMOKE-0 §6.2 named.
3. **PIN-0012's admission.** The plan producer still refuses a mutable observed
   `ProgramData` at plan time (`plan.rs`'s `role_deployment`, the refusal SMOKE-0
   §3.3 exercised with real devnet bytes). That is the surface decision 0012's
   implementing lane owns; until it lands, `prepare
   --ROLE-observed-programdata` cannot describe the mutable substrate 0012
   calls for, and the driver's `substrate` stage has nothing to compare against.
   The driver is built to follow it: the detector reads observed slot **and**
   authority and reports both.
4. **A funded payer, and it is not a faucet.** The preflight prices the run
   exactly (§8) and names the shortfall in lamports. The driver never airdrops.
5. **`sbf_build_diagnostics_total = 0`** — still SMOKE-0 §6.5's item, still the
   dealer-accelerator/hot_v3 seam, still not this lane's.

---

## 8. The measurements

### 8.1 Live devnet preflight (read-only, `api.devnet.solana.com`)

Whole run: **9.2 s wall, rc=0**, paced, every call through the read-only
allowlist. Plan under test: a local campaign's plan, whose role program ids are
local rehearsal ids — so `absent` across the board is the **correct** answer and
is what proves the detectors run rather than assume.

| stage | state |
|---|---|
| `substrate` | absent |
| `publication` | absent |
| `initialize` | absent |
| `activation` | absent |

Wallet arithmetic against **devnet's own live rent**, not a hardcoded formula:

| line | lamports |
|---|---:|
| payer balance (a throwaway key, never funded) | 0 |
| nine infrastructure record bodies | 23,942,400 |
| infrastructure profile | 1,893,120 |
| release activation cache | 1,893,120 |
| estimated fees (200 tx × 5,000, measured-profile: SMOKE-0 read the recent-fee page as all zeros) | 1,000,000 |
| **required** | **28,728,640** |
| **shortfall** | **28,728,640** |

**≈ 0.0287 SOL** to take a deployed substrate from bare programs to an activated
release set — against the ~31.7 SOL of deploy rent that decision 0012 now parks
as recoverable working capital rather than burns.

### 8.2 The refusal rails, executed

Every one of these is a real invocation of the shipped binary:

| invocation | refusal |
|---|---|
| devnet, no acknowledgment | names the flag **and the exact hash to type** |
| `api.mainnet-beta.solana.com` **with** the devnet acknowledgment | "host names mainnet … refused unconditionally" |
| an innocuous host, acknowledged with **mainnet's own genesis hash** | "no flag, no environment variable and no spelling of a URL admits it" |
| `http://` (plaintext) to devnet | "not something to hand to a plaintext hop on a public network" |
| loopback **with** the acknowledgment | "one of the two is a mistake and this refuses rather than guessing which" |
| `--i-mean-devnet true` | "names a cluster identity rather than a boolean so that a command line copied to another cluster stops being true" |

### 8.3 The local proof: the driver does its own writes

The charter asked for the driver to be proved against a local validator first —
"the driver run with the origin allowlist pointed at localhost exercises every
code path except the network." It does, and the proof is the real flow rather
than a simulation of it:

1. `prepare --core-bootstrap-upgrade-authority <the persisted key's address>
   --record-publication transaction` — the plan's authority is a **keypair file
   on disk**, not an in-memory ephemeral key. This is the devnet shape.
2. The guarded validator launched directly on that plan (genesis install of the
   seven roles), on port 21990 — *not* through the supervisor, so nothing but the
   driver ever writes to this chain.
3. Payer funded (10 SOL, local faucet).
4. `campaign --rpc-url http://127.0.0.1:21990/ --plan … --keypair-ROLE …`.

| step | result |
|---|---|
| preflight before any write | `substrate` **complete**, all seven `slot_pin_holds: true`; `publication` / `initialize` / `activation` absent |
| `--execute --through activation` | **28 transactions**; 9 record bodies finalized through Registry `Begin → Append → Finalize`; Core infrastructure profile initialized at **219,949 CU**; activation refused with `0x1004` (§4.1) |
| spend | 25,975,520 lamports of the 10 SOL — rent for nine records plus the profile plus 28 fees |
| **re-run, preflight** | `substrate` complete, **`publication` complete**, **`initialize` complete**, `activation` absent |
| **re-run, `--execute`** | `substrate: already complete, skipped` · `publication: already complete, skipped` · `initialize: already complete, skipped` → resumed **exactly** at activation |

The last two rows are the lane's central claim, executed: **an interrupted run
resumed from the chain alone.** No state file was written or read at any point.
Zero duplicate transactions, zero re-spent rent.

### 8.4 The regression control

Replacing the loopback rail is exactly the change that could have broken the
thing the rail protected, so the control is the supervisor's own campaign, run
end to end on a separate port (20990) under the refactored `rpc_origin`, with
`record_publication = "transaction"` so the nine bodies go on chain as real
Registry transactions rather than at genesis:

```text
SUPERVISOR COMPLETE: 143 transactions, 34 completed steps
last step: executed DCLTPCA1 after expiry — the source principal is back with
           the party that supplied it, and the source vault …
```

Genesis → record publication → infrastructure init → pre-revocation refusal
proof → Core revocation → five-role activation → late-failure rollback → the
Product graph → `DCLTGMF1` founding → the `DCLTPCA1` expiry-abort lane, all
green. The rail moved; nothing under it did.

### 8.5 Gates

| gate | result |
|---|---|
| successor tool build | clean, **zero warnings** |
| successor tool targeted tests (`cluster::`, `campaign::`, `seed::`, `rpc::`, `runtime::tests::the_rpc_origin`) | **27 passed, 0 failed** |
| journey tool build (the `#[path]` tripwire, with `cluster` and `campaign` added to it) | clean |
| `check-frame-diagnostics.py` against two observed sets | rc=0, zero diagnostics |
| root `cargo check --workspace` | **not run, and it cannot see this lane**: both tools declare their own `[workspace]` and are not root members, and this lane touched no file under `crates/` or `programs/`. Stated as the control rather than run as ceremony. |

The build and tests were run in a detached worktree at `75adf653` with this
lane's sources copied in — the shared tree was mid-edit on another lane's
`crates/dclutch-registry-contract` at the time (a duplicate definition, since
resolved). Same pattern the CRYPTO lane recorded on the board for the same
reason.
