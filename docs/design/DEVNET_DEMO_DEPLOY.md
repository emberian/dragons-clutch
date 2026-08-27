# Devnet demo deploy — the runbook

**Status: PREPARATION ONLY. Nothing here has been executed against devnet.**

This document is written to be followed on a day that has not happened yet. It
contains no deployment, no address anyone should treat as real, and no claim of
devnet evidence. The only network traffic behind it is bounded, logged,
read-only JSON-RPC — see [§5](#5-the-devnet-pyth-wiring) and
`tools/release/devnet-observe.sh`, whose `rpc()` allowlist refuses any method
that is not a read.

Every point at which a key signs, lamports move, or an external system changes
is marked **⚠ REQUIRES EXPLICIT USER AUTHORIZATION**. The complete list of them
is [§8](#8-the-authorization-checklist), and it is the only list. An agent
following this runbook stops at each one.

Read `AGENTS.md` first. Its rule is the operative one: *never sign, submit,
deploy, fund, publish, push, tag, or mutate an external system without explicit
current authorization naming that act.*

---

## 0. What the demo is, and what has to be true for it

`WAVE.md` sets the shape: **the mostly-completed protocol live on devnet,
resolving markets about the state of Solana mainnet.** Majors' prices need no
relayer, because Pyth's devnet deployment already carries mainnet-derived prices
under the existing adapter. That is the wiring this runbook is for.

Reaching it needs three things, in this order, and the ordering is forced by
the protocol rather than by convenience:

1. **Seven Loader V3 programs**, each immutable by the time the protocol reads
   it. Registry and Rent must already be immutable when Core initializes its
   infrastructure profile; Core must still hold an authority at that moment and
   lose it immediately after.
2. **Nine infrastructure record bodies**, published as real transactions,
   because devnet has no genesis to inject them into.
3. **The market campaign** — publication → activation → RentV2 → Found31 →
   `DCLTPCB1` → `DCLTGMF1` — which is already proved end to end on a local
   validator (`WAVE.md`, run 6, `67e441d`).

The third is done. The first two are what this runbook adds, and writing them
down surfaced three blockers that no local run could have found, because a local
run is handed exactly the substrate that devnet does not have. They are
[§7](#7-what-is-still-blocking-deploy-day).

---

## 1. Preconditions

| Fact | Value at the time of writing | How to re-check |
|---|---|---|
| devnet genesis hash | `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` | `tools/release/devnet-observe.sh` |
| devnet `solana-core` | 4.2.1 (feature set 565236538) | same |
| local `solana-cli` | 4.0.2 (`src:549805f3`, Agave) | `solana --version` |
| devnet rent | `min_balance(n) = 890,880 + 6,960·n` lamports, affine, confirmed exact at n = 10⁶ | same |
| budget | **45 devnet SOL banked** (`WAVE.md` standing decision) | — |
| artifacts | one `tools/release/checked-release-candidate.sh` run at the exact deploy commit | — |

The **checked release candidate is the input**, not an optional nicety. Deploy
day starts by producing one at the exact commit being deployed, and every ELF
digest in this runbook's verification steps comes from its `SUMMARY.txt`.

> **The candidate at HEAD is not the one in
> `docs/evidence/CHECKED_RELEASE_CANDIDATE_2026_08_26.md`.** That document says
> to replace it rather than amend it, and it has been overtaken: measured at
> `3b0c5883`, `sbf_build_diagnostics_total` is now **0** — its finding 1 (36
> stack-frame diagnostics from the dealer accelerator's Trading
> monomorphization) is **fixed**, and that artifact shrank from 599,360 to
> 211,048 bytes. Trading grew from 1,287,728 to 1,384,680. Regenerate before
> deploying and do not carry the old digests forward.

### Program identity

Seven program addresses are needed and **each requires a keypair the deployer
holds**. Nothing in this repository has ever minted one: the checked-release
candidate's addresses are SHA-256 of a fixed domain string and explicitly have
no private key, and the local bootstrap takes program ids as arbitrary
user-supplied base58 with no key material at all
(`plan::validate_program_ids`). Generating seven program keypairs is itself an
authorization point — see [§8](#8-the-authorization-checklist) item 1 — and once
generated they must be treated as deploy secrets, because whoever holds one can
claim that address before you do.

Vanity addresses are a real option here and cost only grind time, but the
address must be fixed *before* the record bodies are minted, because it is an
input to every artifact release digest.

---

## 2. Program deployment: buffer → deploy → verify → revoke

### 2.1 Why the order is what it is

`solana program deploy` in one shot is the wrong instrument for a 33-SOL,
one-way spend. Split it, so that the step that spends recoverable money is
separated from the step that makes it unrecoverable, with a verification
between them:

```text
  write-buffer      recoverable      the ELF lands on chain, closeable
       ↓
  verify buffer     free             its bytes are the bytes you built
       ↓
  deploy            recoverable      Program + ProgramData exist, mutable
       ↓
  verify accounts   free             the chain holds what the manifest says
       ↓
  set --final       ← POINT OF NO RETURN, rent is now permanently burned
```

**Serial, one role at a time.** Peak lamport demand under a serial deploy is
bounded by the final resident total: a buffer holding role *k*'s ELF costs
`rent(37 + n)`, which is eight bytes *less* than the `rent(45 + n)` ProgramData
it becomes, and the loader drains the buffer to the payer inside the same
`DeployWithMaxDataLen` instruction that creates the ProgramData. Parallel
deployment does not raise the ceiling either, but it does raise the blast
radius: seven simultaneous buffers is seven ways for a partial failure to
strand rent.

Deploy order is forced by [§2.4](#24-the-revocation-order-is-a-protocol-fact).

### 2.2 Per role

⚠ **REQUIRES EXPLICIT USER AUTHORIZATION** — every command in this section
writes to devnet.

```sh
ROLE=registry                       # then rent, custody, resolution, claims, trading, core
ELF=$WORK/elf/$ROLE.so
URL=https://api.devnet.solana.com

# 1. buffer. Recoverable: `solana program close --buffers` reclaims it.
solana program write-buffer "$ELF" --url "$URL" \
    --buffer "$KEYS/$ROLE-buffer.json" --output json | tee "$OUT/$ROLE-buffer.json"

# 2. verify the buffer holds the artifact you built, BEFORE paying to deploy it.
#    Buffer layout is 37 bytes of Loader metadata then the ELF.
solana account "$BUFFER" --url "$URL" --output-file "$OUT/$ROLE-buffer.bin"
tail -c +38 "$OUT/$ROLE-buffer.bin" | shasum -a 256      # == <role>_elf_sha256

# 3. deploy from the verified buffer. --max-len defaults to the buffer's length,
#    which is what we want: no headroom, because these are never upgraded.
solana program deploy --url "$URL" --buffer "$BUFFER" \
    --program-id "$KEYS/$ROLE-program.json" --output json | tee "$OUT/$ROLE-deploy.json"

# 4. verify the deployed accounts and RECORD THE DEPLOYMENT SLOT.
#    That slot is a protocol input -- see §7 blocker A.
solana program show "$PROGRAM_ID" --url "$URL"
```

### 2.3 Verification between deploy and revocation

This is the last moment anything is recoverable, so check all of it:

- `Program` account: owned by `BPFLoaderUpgradeab1e…`, executable, **exactly 36
  bytes**, `u32 LE` discriminant `2`, bytes `[4..36]` equal to
  `find_program_address([program_id], loader)`.
- `ProgramData`: owned by the loader, **not** executable, discriminant `3`,
  `space == 45 + elf_len` exactly, authority tag `1` at byte 12 with your
  deployer key at `[13..45]`, and `sha256(bytes[45..]) == <role>_elf_sha256`.
- **Deployment slot** at `[4..12]` — write it down. Seven of these are inputs to
  [§3](#3-the-transaction-campaign).

`tools/release/devnet-observe.sh` decodes exactly this shape for the Pyth
programs and is the reference for the byte offsets.

### 2.4 The revocation order is a protocol fact

⚠ **REQUIRES EXPLICIT USER AUTHORIZATION.** Each of these is irreversible.

`programs/dclutch-core-sbf/src/infrastructure.rs::process_initialize`
authenticates the Registry and Rent artifacts through
`require_pinned_immutable_deployment`, which refuses unless the observed
ProgramData carries **no** upgrade authority — while
`authenticate_current_core_upgrade_authority` in the same function requires Core
to *still hold* an authority that signs. Both conditions are live in one
instruction.

The minimal constraint is therefore only: *Registry and Rent immutable before
Core init; Core mutable during it and immutable before activation; Claims,
Trading, Resolution and Custody immutable before their own activation.* The
schedule below over-constrains that deliberately, so that **at most one upgrade
authority is live at any moment** and it is Core's ephemeral one. A deploy day
with six spare authorities lying around is a deploy day with six ways to lose
the protocol.

```text
1. Registry     deploy → verify → set-upgrade-authority --final
2. Rent         deploy → verify → set-upgrade-authority --final
3. Custody      deploy → verify → set-upgrade-authority --final
4. Resolution   deploy → verify → set-upgrade-authority --final
5. Claims       deploy → verify → set-upgrade-authority --final
6. Trading      deploy → verify → set-upgrade-authority --final
7. Core         deploy → verify → KEEP THE AUTHORITY
                ... §3 publication and Core infrastructure init happen here ...
8. Core         set-upgrade-authority --final
```

```sh
solana program set-upgrade-authority "$PROGRAM_ID" --url "$URL" \
    --upgrade-authority "$KEYS/$ROLE-authority.json" --final
```

The Core authority is **ephemeral by design**. The local supervisor generates it
with `Keypair::new()`, never writes it to disk, and records
`private_key_persisted: false`. On devnet it must exist as a file long enough to
sign two transactions — infrastructure init and its own revocation — and should
be destroyed immediately after step 8, not archived.

### 2.5 What revocation leaves behind — measured

The loader's `Some → None` serialization sets byte 12 to `0` and **leaves bytes
`[13..45]` holding the former authority key** as inactive storage. The ELF still
starts at byte 45. The local plan pins this exact poststate
(`plan::loader_programdata_bytes_after_revoke`, which flips byte 12 and nothing
else) and the supervisor verifies it.

This lane measured it directly rather than trusting the pin. On a throwaway
`solana-test-validator 4.0.2` (isolated port, ephemeral local keys, torn down
afterwards), deploying `rent.so` and then running
`solana program set-upgrade-authority --final`:

```text
before --final   space=152357 tag=3 slot=531 authtag=1
                 bytes[13..45]=8d2e3468ff9086da37c9b8b5504cae7f299fb3e45807d4c2942835298cf08aa1
after  --final   space=152357 tag=3 slot=531 authtag=0
                 bytes[13..45]=8d2e3468ff9086da37c9b8b5504cae7f299fb3e45807d4c2942835298cf08aa1
                                ^ unchanged: exactly the former authority's pubkey
```

Only byte 12 moved. See [§7 blocker B](#blocker-b-no-checked-manifest-can-describe-a-deployed-then-revoked-program).

---

## 3. The transaction campaign

Everything below this line is transactions. Since W1e/W1f the whole campaign is
transaction-driven; the genesis-injected parts were only the program deploys
themselves and the local Pyth fixture — and on devnet even those become real
deploys and the already-live Pyth.

⚠ **REQUIRES EXPLICIT USER AUTHORIZATION** — the entire section.

### 3.0 Mint the nine record bodies from observed facts

**This step does not exist in any local run and it cannot be skipped.** Each
`ArtifactReleaseV1` binds an exact `deployment_slot`, checked on chain by
`require_loader_linkage` → `authenticate_deployment`. Locally that slot is `0`
because genesis installs at slot 0. On devnet it is whatever slot each deploy
landed in, which is unknowable until §2 has run.

So the nine bodies — seven `ArtifactReleaseV1`, one `ExecutionReleaseSetV1`, one
`PythReleaseV1` — are **downstream of deployment**, and so is everything derived
from them: each record's PDA (a function of schema and content digest), the
release-set digest, the activation cache PDA, and the infrastructure profile.

See [§7 blocker A](#blocker-a-the-bootstrap-hardcodes-deployment_slot--0).

### 3.1 Publication

Nine `Begin → Append → Finalize` sequences through the Registry, one per body,
each landing at `find_program_address([RAW_RECORD_PDA_SEED_V1, schema, digest],
registry)`. Publication is permissionless; principal comes from the sponsoring
System wallet and the temporary staging cursor commits that same wallet as its
only refund destination.

Measured offline against the real HEAD artifacts (`plan-tx.json`, this lane):

| record | bytes | rent |
|---|---:|---:|
| `execution_release_set` | 336 | 0.003229440 SOL |
| `pyth_release` | 440 | 0.003953280 SOL |
| 7 × `<role>_artifact_release` | 216 each | 0.002394240 SOL each |
| **total** | **2,288** | **0.023942400 SOL** |

`tools/local-validator/bootstrap/successor` gained
`--record-publication transaction` for exactly this (`fab6aaf`). Under it the
nine bodies leave genesis, the supervisor publishes each through the same
permissionless path a market record uses, and the launcher's plan gate asserts
positively that every genesis account key starts with `loader.`. A plan test
prepares both modes over the same inputs and requires every raw address,
staging address, digest and body to be **equal** — a record's coordinate is a
function of schema and content, never of who wrote the bytes. Confirmed with
the real ELFs: 23 genesis accounts under `genesis`, **14** under `transaction`,
identical coordinates.

### 3.2 Core infrastructure initialization

One transaction, signed by Core's still-live ephemeral authority, creating the
sole 144-byte `ProtocolInfrastructureProfileV1` at its PDA under Core from the
finalized Registry and Rent artifact records. Rent 0.001893120 SOL.

Then, and only then, **Core revocation** (§2.4 step 8).

### 3.3 Activation, one role per transaction

Five `RegistryInstructionV1::ActivateRole` transactions — Core, Claims, Trading,
Resolution, Custody. **One role per transaction is a hard requirement, not a
style choice**: first admission hashes the whole ELF at roughly one compute unit
per two bytes, so Trading alone is ~692k CU at 1,384,680 bytes and five real
artifacts in one transaction cannot fit under the 1,400,000 maximum. A partially
activated cache cannot decode, so no reader can consume a half-activated set.

Worst measured single activation locally: **Trading, 710,601 CU** — and that was
at 1,287,728 bytes, i.e. 0.552 CU per byte all-in. HEAD's Trading is 1,384,680
bytes, which *projects* to ≈764,000 CU. That is a projection from one
measurement, not a measurement; **re-measure it at the deploy commit.** It has
~55% headroom today and Trading is the artifact most likely to keep growing.

### 3.4 Market lifecycle

Unchanged from the local campaign, which is where its evidence comes from
(`WAVE.md` run 6, `67e441d`; `tools/local-validator/bootstrap/successor/README.md`):

| step | route | measured CU (local) |
|---|---|---:|
| Token-2022 collateral Mint + funded wallet | Token-2022 | — |
| Realm / Product graph / Source / recovery / manifest / basis publication | Registry | — |
| same-slot pre-credit projection, then `RentCreditV2` create + finalized reacquire | Rent | — |
| finalized address lookup table for the Found frame | Address Lookup Table | — |
| `Found31` canonical Market creation (v0 tx over the ALT) | Core | 223,540 |
| `DCLTPCB1` projected-Custody prestate, four stages | Trading → Custody | 754,119 |
| five pre-fundings, one transaction | System | — |
| `DCLTGMF1` Lock → Found → Realize → Claims → **Open last** | Trading → all | 1,184,132 |

**Compute budget on the wire.** Every transaction carries
`SetComputeUnitLimit(1,400,000)`. `DCLTPCB1` and `DCLTGMF1` — and only those two
— additionally carry `RequestHeapFrame(262,144)`, because Trading owns its
entrypoint and allocator and re-derives the grant from the instructions sysvar
in the *top-level instruction's own account list*. Both routes present that
sysvar in their fixed prefix. A transaction that omits either is not the
transaction that was measured.

**The five pre-fundings.** Nothing in the protocol funds these; Core and Claims
`allocate` + `assign` and never transfer. Two must be **exact**:

| account | requirement |
|---|---|
| Market | `lamports == rent.minimum_balance(352)` **exactly** — over-funding refuses |
| one-shot permit | `lamports == rent.minimum_balance(608)` **exactly** |
| Claims aggregate | ≥ `rent.minimum_balance(256 + 8·claim_count)` |
| founder Position | ≥ `rent.minimum_balance(128 + 8·claim_count)` |
| Claims admission | ≥ `rent.minimum_balance(512)` |

All three Claims balances are digest-bearing: Core reads them at the Found stage
and folds them into the Claims request committed inside the permit. **A
pre-funding one lamport off does not overpay — it moves a digest and refuses at
Claims.** Compute them from the cluster's own
`getMinimumBalanceForRentExemption`, never from a constant carried across
clusters.

### 3.5 The compute margin is the live risk

`DCLTGMF1` went from **1,184,132 CU (84.6% of maximum) to 1,278,747 CU (91.3%)
in one evening** at the end of 2026-08-26, entirely from other lanes' concurrent
Core/Claims/Trading changes, with nothing watching it. There is no headroom to
buy: the campaign already requests the maximum. **Re-measure `DCLTGMF1` at the
exact deploy commit and refuse to deploy above ~95%**, because the failure mode
is a hard refusal at the ceiling with no partial result — and on devnet the
retry costs another founding's rent.

The CU-BUDGET lane owns the checked-in budgets. Deploy day should read them.

---

## 4. Rent mathematics against the 45 SOL budget

Devnet rent is affine and was read from the cluster, not assumed:
`min_balance(n) = 890,880 + 6,960·n`, confirmed exact at n = 10⁶. The
`128`-byte account overhead and the `2.0`-year exemption are visible in it
(`128 · 3480 · 2 = 890,880`; `3480 · 2 = 6,960`).

Loader V3 geometry per role: `Program` is **36** bytes, `ProgramData` is
**45 + elf_len**, and `--max-len` defaults to the ELF length, so there is no
2× allocation. The live devnet Pyth programs show the shape (`space = elf + 45`
on all three), but they could have been deployed with an explicit `--max-len`,
so this lane measured the CLI's actual default instead of inferring it — a 2×
default would have made the seven-role budget ~65 SOL and this whole plan wrong.

**Measured**, `solana-cli 4.0.2` deploying `rent.so` (152,312 B) to a throwaway
local validator:

```text
Data Length: 152312 bytes          ← the ELF length, not twice it
ProgramData space: 152357          ← 45 + 152312
ProgramData balance: 1.0612956 SOL ← 890,880 + 6,960 × 152,357 exactly
payer spent: 1,063,212,040 lamports
             = 1,062,437,040 rent + 775,000 fees (155 transactions)
```

The rent matches the table below to the lamport, and 155 transactions is the
predicted 151 writes plus buffer creation, deploy, and confirmations.

### 4.1 Per-artifact, at HEAD `3b0c5883`

| role | ELF bytes | ProgramData bytes | writes | write fees | **final rent** |
|---|---:|---:|---:|---:|---:|
| registry | 220,728 | 220,773 | 219 | 0.001095 | **1.538612** |
| rent | 152,312 | 152,357 | 151 | 0.000755 | **1.062437** |
| custody | 355,760 | 355,805 | 352 | 0.001760 | **2.478435** |
| resolution | 527,504 | 527,549 | 522 | 0.002610 | **3.673773** |
| claims | 1,073,376 | 1,073,421 | 1,061 | 0.005305 | **7.473042** |
| trading | 1,384,680 | 1,384,725 | 1,369 | 0.006845 | **9.639718** |
| core | 1,007,224 | 1,007,269 | 996 | 0.004980 | **7.012625** |
| **seven roles** | **4,721,584** | | **4,670** | **0.023350** | **32.878643** |
| general-accelerator | 193,968 | 194,013 | 192 | 0.000960 | 1.352363 |
| dealer-accelerator | 211,048 | 211,093 | 209 | 0.001045 | 1.471240 |
| series-shadow | 111,056 | 111,101 | 110 | 0.000550 | 0.775295 |
| **all ten** | **5,237,656** | | **5,181** | **0.025905** | **36.477541** |

Write counts assume ~1,012 payload bytes per `Write` transaction (the legacy
1,232-byte packet minus one signature, three keys, blockhash, and the Write
instruction envelope) at the 5,000-lamport base fee. Fees are rounding error
next to rent; they are listed so nobody is surprised by ~5,000 transactions.

### 4.2 The budget

| line | seven roles | all ten |
|---|---:|---:|
| program rent (permanent) | 32.878643 | 36.477541 |
| write transaction fees | 0.023350 | 0.025905 |
| nine infrastructure records | 0.023942 | 0.023942 |
| infrastructure profile | 0.001893 | 0.001893 |
| market lifecycle accounts (Market, permit, aggregate, Position, admission, Hoard vault, Custody replay, projection) | ~0.029 | ~0.029 |
| **total** | **≈ 32.957** | **≈ 36.558** |
| **remaining of 45 SOL** | **≈ 12.04** | **≈ 8.44** |

The market lifecycle line excludes the Registry publication records for the
Realm / Product graph / Source / recovery policy / capability manifest / linked
basis and the address lookup table, which are each a few hundred bytes and
recoverable. Budget ~0.1 SOL for the whole campaign and it will be generous.

**The budget is not the constraint. The irreversibility is.** 45 SOL buys the
ten-artifact deployment with ~8 SOL spare, or the seven-role deployment with
~12. It does **not** buy two of either.

### 4.3 Recycling: what can come back, and when it stops

> A Loader V3 program whose upgrade authority is `None` **can never be closed.**
> `UpgradeableLoaderInstruction::Close` requires an authority signature and an
> immutable ProgramData has no authority to sign.

**Measured, not assumed.** On the same throwaway validator, `solana program
close` against the program that had just been set `--final`:

```text
Error: Program authority None does not match Some(AW7MFJYyxBoWzguBv8h3QRQKKLDEy1wb9eTi9xpPwWMN)
```

The same command against a *mutable* deployment of the same ELF succeeded and
reported `1.0612956 SOL reclaimed`.

And dClutch *requires* that immutability — `CheckedInfrastructureV1::validate`
refuses a mutable Core/Registry/Rent, and activation refuses a role whose
ProgramData still carries an authority. **The protocol's correctness condition
is the same event as the loss of the money.**

Each role therefore has a **recycle window** that opens when its buffer is
created and closes when its authority is revoked. Inside the window everything
is recoverable; outside it, nothing is.

| what | reclaimable? | how |
|---|---|---|
| orphan buffer from a failed write | **yes**, in full | `solana program close --buffers --authority …` |
| deployed but not yet revoked program | **the ProgramData rent only** — see below | `solana program close <ID> --authority …` |
| the 36-byte `Program` account after a close | **NO** | it survives the close, still `tag = 2`, executable, holding 0.00114144 SOL |
| revoked (immutable) program | **NO — permanently burned** | — |
| address lookup table | yes | deactivate, wait the cooldown, close |
| `RentCreditV2`, market accounts | yes | protocol closure routes |
| finalized Registry records | no | immutable by construction |

`tools/release/devnet-recycle.sh` computes this. It plans by default with
bounded reads and prints exact commands; `--execute` refuses without
`--authorization` carrying text naming the act, and refuses mainnet's genesis
hash outright. Verified against the live devnet Pyth programs: it derived
`3UV7w2yT…` and `9nxngQjx…` offline and matched the observed lamport balances
exactly.

**Practical consequence for a rehearsal on devnet.** If you want a devnet dress
rehearsal that you can *undo*, run it with the programs left mutable, accept
that infrastructure init and activation will refuse, and recycle the ~33 SOL.
The moment you revoke, that is the deploy. There is no third option.

---

## 5. The devnet Pyth wiring

### 5.1 Observed, 2026-08-27T08:53Z — 12 bounded read-only calls

Every call is logged by `tools/release/devnet-observe.sh` (`--out` writes
`rpc-reads.log`, one UTC timestamp / method / params line per call). No writes,
no signing, no keypairs, no airdrops. The complete log for the observation this
section reports:

```text
08:53:39Z  getGenesisHash                      []
08:53:40Z  getVersion                          []
08:53:40Z  getEpochInfo                        []
08:53:40Z  getMultipleAccounts                 [router, receiver, push-oracle]                     finalized
08:53:40Z  getAccountInfo                      9hLWdeVh…  dataSlice(0,45)                          finalized
08:53:40Z  getAccountInfo                      3UV7w2yT…  dataSlice(0,45)                          finalized
08:53:40Z  getAccountInfo                      9nxngQjx…  dataSlice(0,45)                          finalized
08:53:41Z  getMultipleAccounts                 [Config, GuardianSet[0], bridge config, SOL/USD]    finalized
08:53:41Z  getMinimumBalanceForRentExemption   [0]
08:53:41Z  getMinimumBalanceForRentExemption   [1]
08:53:41Z  getMinimumBalanceForRentExemption   [1000000]
08:53:41Z  getSignaturesForAddress             7AviUf9nL…  limit 1000                              finalized
```

Twelve calls, two seconds, one endpoint. The three ProgramData reads take only
the 45-byte header — this lane never re-fetched the megabyte-scale ELF bodies,
because `PROVENANCE.md` already owns those digests and re-hashing them would be
a megabyte of traffic to re-learn a fact the repository holds.

**Every fact pinned by `fixtures/pyth/upgraded-2026-08-26/PROVENANCE.md` on
2026-08-26 still reproduces exactly.** Nothing moved:

| role | program id | ProgramData | deploy slot | upgrade authority |
|---|---|---|---:|---|
| router (Wormhole receiver) | `HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL` | `9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x` | 460,336,290 | `upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr` |
| Pyth Solana receiver | `rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp` | `3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX` | 460,336,311 | same |
| Pyth push oracle | `pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou` | `9nxngQjxBGUZ3ajfqoTrpiuDBVfztXCQVDuWDAw52Gew` | 460,336,332 | same |

All three `Program` accounts are 36 bytes, tag 2, executable, loader-owned.

| account | address | observed |
|---|---|---|
| receiver `Config` | `H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye` | 370 B, sha256 `23a7a19cf60c1fda8f070323fb8f1013a32851b0921fb7b2ac085990cbfaa37a` |
| `GuardianSet[0]` | `CJHmJw4FuvLTUfPsYepyVCQkUR8qv1AtZbkwsS36hEcd` | 124 B, sha256 `8f11fb97aa312c18721cac3573a86704ec94cf6b27b26ac4eae9f94b83903736` |
| bridge config | `GPhDjebMkciFeemuNGaUn5RsmxauQL7UZArqRDjCSZSW` | 24 B, sha256 `e1fc75700784edad532950e20ef754d912134c5aa788a52c77befc38b4320541` |
| **SOL/USD `PriceUpdateV2`** | **`7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE`** | 134 B, **owned by the receiver** `rec2HH…`, `verification_level = Full`, `expo = -8` |

The receiver `Config` still reads `governance_authority =
7g4Los4WMQnpxYiBJpU1HejBiM6xCk5RDFGCABhWE9M6` (devnet-specific), one data source
`chain 26 (Pythnet) / 6R92oFT6UiP2xWZBjTbwAkHzFCLy5BhWnNh6m83ndhZR`,
`single_update_fee = 0`, `minimum_signatures = 3`. `GuardianSet[0]` still holds
**five** 20-byte keys with `expiration_time = 0`, and the five key bodies are
unchanged.

**Trust root: a 3-of-5 Pyth-controlled multisig.** Strict majority of five is 3,
which equals the receiver's own `minimum_signatures`, so `PythReleaseV1`'s
`count / 2 + 1` rule and the receiver policy coincide under this generation.
That is a fact about *this* generation, not a general one — under the previous
19-key set they were 10 and 5. "Zero new trust" for a Pyth-sourced market
remains exact, and what that trust *is* has become materially smaller. It
belongs in the Product's disclosure, not a footnote.

At the read, SOL/USD was `104.502535` published `2026-08-27T08:53:25Z`, **16
seconds old**.

*(Corrected while writing this: the bridge-config layout is `guardian_set_index
u32 | last_lamports u64 | guardian_set_expiration_time u32 | fee u64`. An
earlier decode in this lane read the expiry and fee at the wrong offsets and
printed nonsense; the account digest was identical to the pinned one throughout,
so the bytes never moved — only my reader was wrong.)*

### 5.2 A release is per cluster, and the cluster is a bound fact

`PythReleaseV1` carries a `cluster_id`, and it must be **the genesis hash**,
never inferred by the adapter:

| cluster | genesis hash |
|---|---|
| `mainnet-beta` | `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d` |
| **`devnet`** | **`EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`** |

The three ELFs are byte-identical across clusters. What is not, and what a
release actually binds, is per cluster: deployment slot, upgrade authority,
complete ProgramData body digest, `Config.governance_authority`, complete
`Config` digest, and the `GuardianSet[0]` account digest (`creation_time`
differs by 104 s).

**The devnet release row does not exist.** `dclutch_pyth_svm::release::
PRODUCTION_RELEASES` is `[PythReleaseV1; 0]` — deliberately empty — and
`SyntheticLocalReleaseV1` is a distinct type with *no conversion* to
`PythReleaseV1`, so the lab release cannot become a production one by accident.
A `PythReleaseV1Input` for devnet is:

| field | devnet value |
|---|---|
| `cluster_id` | devnet genesis hash |
| `receiver_program` / `receiver_programdata` / `receiver_config` | `rec2HH…` / `3UV7w2yT…` / `H3R4M45f…` |
| `router_program` / `router_programdata` | `HDw2E7…` / `9hLWdeVh…` |
| `config_digest` | `23a7a19c…` (devnet-specific) |
| `receiver_abi_id` | `c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64` |
| `router_abi_id` | `f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb` |
| `receiver_deployment_slot` / `router_deployment_slot` | 460,336,311 / 460,336,290 |
| `guardian_set_count` / `required_guardian_count` | **5 / 3** |

Minting that row is a **protocol fact owned by the Pyth adapter lane**, not
something a deploy runbook or host tooling should author. It is
[§8](#8-the-authorization-checklist) item 6's precondition.

### 5.3 Staleness is a measurement with a date

*measured-profile, re-measured 2026-08-27 in this lane over one page of 1,000
finalized signatures on `7AviUf9nL…`, an 86.43-hour window, 999 gaps:*

| | devnet |
|---|---:|
| p50 | 314 s |
| p90 | 321 s |
| p99 | 325 s |
| **max observed gap** | **4,784 s (1 h 19 m 44 s)** |

This reproduces the PY lane's 2026-08-27 figures (p50 313, max 4,784) over a
shorter window, and the maximum is the same discrete event:
2026-08-25T08:42:02Z → 10:01:46Z. The next largest gap was 354 s, so it was one
outage, not drift. Mainnet, for contrast, runs p50 7 s / max 21 s.

**A `WindowSpecV1.max_age_seconds` is a maximum, not a median.** The n = 12
sample that preceded this suggested 400 s; that bound would have refused every
read for 79 consecutive minutes on 2026-08-25. A devnet majors Product must
either carry `max_age_seconds ≥ 4,784` — which is a very weak freshness
guarantee and must be disclosed as one — or treat provider silence as an
**expected state with a funded permissionless failure path**, which is what the
capability manifest's three Resolution funding entries exist for.

**Both figures remain measured-profile and provisional** (`AGENTS.md`: every
fixed bound is labeled, and provisional bounds require a lifting plan).

**The lifting plan.** A finite observation window is not a bound, and a longer
one-off is not the fix — the 7-day window and the 3.6-day window found the same
maximum, which is evidence that one-off sampling saturates rather than
converges. Lift it by *continuous observation with a running maximum*:

1. Run `tools/release/devnet-observe.sh --cadence` on a schedule (hourly is
   ample against a 314 s median), appending each window's max to a committed
   series.
2. Keep the **running maximum since a named start date**, not the latest
   window's. Publish the start date with the number, because a maximum without a
   window length is not a measurement.
3. Set `max_age_seconds` from the running maximum plus an explicit margin, and
   re-label it `measured-profile` with that date every time it moves.
4. It becomes chain-derived only if Pyth ever publishes an on-chain liveness
   commitment. Until then it stays provisional, and the funded failure path —
   not the bound — is what makes the Product safe.

---

## 6. The dry-run gate

**What it proves.** The full transaction-only campaign against a local validator
started with *nothing but the seven deployed programs and real-shape Pyth* — no
genesis protocol state whatsoever. It is the rehearsal for §3.

**Why it was not previously possible.** Until `fab6aaf`, `plan::prepare`
unconditionally wrote 23 genesis accounts: 14 Loader V3 accounts *and nine
finalized Registry record bodies*. Three separate layers required exactly 23
(`runtime::validate_plan`, the launcher's jq gate, and a plan test). Every
campaign to date has therefore been handed the ExecutionReleaseSet, all seven
ArtifactRelease bodies, and the Pyth release body as account fixtures — a
substrate **no cluster has**.

**The change.** `--record-publication transaction` on `prepare` (and the
matching optional run-spec field, and `--record-publication` on
`tools/gauntlet/run.sh`) removes those nine from genesis and makes the
supervisor publish each through the Registry's permissionless `Begin → Append →
Finalize` path before Core initialization or activation can read them. Absent
means `genesis`, so every existing spec is byte-for-byte unchanged.

**Offline result (real HEAD artifacts, both modes prepared side by side):**

| | genesis | transaction |
|---|---:|---:|
| genesis accounts | 23 | **14** (all `loader.*`) |
| records | 9 | 9 |
| record coordinates | — | **identical to genesis mode** |

The identity is the point: a record's address is a function of schema and
content, so moving the writer moves nothing the protocol can observe.

**On-chain result:** *(see §6.1)*

**Reproduce:**

```sh
tools/gauntlet/run.sh --work /private/tmp/da2-gauntlet \
    --commit 90d7688dd9847f4a248415fb65237087882cd61e \
    --record-publication transaction
```

The mode is folded into `SPEC_INPUT_DIGEST`; without that, switching modes would
match the previous stamp and silently reuse a campaign that ran the *other*
shape — a rehearsal reporting on a substrate it did not use.

### 6.1 Result

*Filled in from the run; see the lane report.*

---

## 7. What is still blocking deploy day

Three, all found by writing this document rather than by running anything,
because each is invisible to a local run by construction.

### Blocker A: the bootstrap hardcodes `deployment_slot = 0`

`tools/local-validator/bootstrap/successor/src/plan.rs:597-607` builds every
`ArtifactReleaseV1` with `deployment_slot` literal `0`. That is correct for
genesis installs and wrong for every real deploy.

It is load-bearing on chain, not just in evidence:
`programs/dclutch-core-sbf/src/infrastructure.rs::require_loader_linkage` reads
`programdata_view.deployment_slot()` into the `DeploymentObservationV1` that
`release.authenticate_deployment(observation)` checks, and
`crates/dclutch-registry-contract/src/artifact.rs:250` is unambiguous:

```rust
if observed.deployment_slot != self.deployment_slot {
    return Err(Error::DeploymentSlotMismatch);
}
```

A record claiming slot 0 against a ProgramData deployed at slot 488,7xx,xxx
refuses. The measured local deploy landed at slot 167 and its redeploy at 531 —
the value is not even stable across two runs on the same machine, so it cannot
be pre-committed.

**Fix**: a per-role `--<role>-deployment-slot` on `prepare` and a matching
run-spec field, defaulting to 0 so local runs are unchanged. Small, additive,
and it must land before deploy day. **Not done in this lane** — it is a protocol
input plumbing change and the value cannot be known until §2 has run, so it
belongs with whoever owns the deploy execution.

Note the ordering this forces and that §3.0 states: deploy all seven → revoke →
observe seven slots → mint the nine bodies → publish → initialize → activate.

### Blocker B: no checked manifest can describe a deployed-then-revoked program

`dclutch_release_tool::loader_v3_programdata_account_data_v1` writes the
authority tag and key when `--upgrade-authority` is given, and **zeros
`[12..45]`** when it is not. A real revoked program has tag `0` at byte 12 and
the *former authority key retained* at `[13..45]` (§2.5).

So `loader-accounts` can express "mutable, authority A" and "immutable, never
had an authority", but **not** "immutable, formerly A" — which is the only shape
a devnet deployment can be in. Consequently every checked manifest's
`programdata_account_sha256` is wrong for a real deployment, in two independent
ways: the retained authority bytes and the nonzero deployment slot.

Measured side by side on the throwaway validator (§2.5):

```text
observed, after --final        tag=3  slot=531  authtag=0  [13..45]=8d2e3468…08aa1
loader-accounts, no authority  tag=3  slot=0    authtag=0  [13..45]=0000…0000
```

This does **not** break the chain. `authenticate_deployment`
(`crates/dclutch-registry-contract/src/artifact.rs:229-260`) compares identity,
the ProgramData link, both owners, both executable flags, the deployment slot,
the **ELF digest**, and the upgrade authority — and nothing else. There is no
whole-account digest in the observation, so the retained bytes are invisible to
it. It **does** break the frontend's
`authenticateDeployment`, which contract item 4 of
`docs/evidence/CHECKED_RELEASE_CANDIDATE_2026_08_26.md` specifies as a byte-exact
geometry-and-digest gate. Every devnet role would be refused by the browser.

**Fix**: give `loader-accounts` a `--revoked-authority <hex32>` that emits tag
`0` with the retained key, and produce the deploy-day manifests from *observed*
accounts with observed slots — at which point the manifests stop being
predictions and become deployment observations, one rung up the evidence ladder.
That is the same regeneration the checked-release candidate already says is the
cheapest next step on this path.

### Blocker C: the frontend still conflates Core with the Registry program

Unchanged and already specified: `lib/releaseRegistry.ts`'s
`prepareRegistryActivation` throws unless `releaseSet.roles.core.program ===
registryProgram`, which is the opposite of what `lib/infrastructure.ts` and the
on-chain contracts require. Any honest seven-program release set — including a
devnet one — is refused today. Contract items 1–5 in
`docs/evidence/CHECKED_RELEASE_CANDIDATE_2026_08_26.md` specify the fix.

---

## 8. The authorization checklist

One list. Each line is a distinct act and needs its own explicit, current
authorization naming that act. Nothing below has been done.

| # | act | irreversible? | cost |
|---|---|---|---|
| 1 | **Generate seven program keypairs** (plus buffer and authority keys) and store them as deploy secrets | no, but the addresses become inputs to every digest | — |
| 2 | **Fund the deploy payer** on devnet | no | — |
| 3 | **Write seven buffers** to devnet (~4,670 transactions) | no — buffers are closeable | ~32.9 SOL held, 0.023 SOL fees |
| 4 | **Deploy seven programs** from verified buffers | no — mutable programs are closeable | net ~0.008 SOL |
| 5 | **Revoke Registry, Rent, Custody, Resolution, Claims, Trading to `--final`** | **YES — 25.87 SOL becomes permanently unrecoverable** | — |
| 6 | **Publish nine infrastructure records** (needs the devnet `PythReleaseV1` row to exist first) | records are immutable; rent is not reclaimable | 0.024 SOL |
| 7 | **Initialize the Core infrastructure profile** | no | 0.002 SOL |
| 8 | **Revoke Core to `--final`** | **YES — a further 7.01 SOL permanently unrecoverable** | — |
| 9 | **Activate five roles**, one transaction each | no | — |
| 10 | **Run the market campaign** through `DCLTGMF1` to an Open Market | Market accounts are closeable through protocol routes | ~0.03 SOL |
| 11 | **Deploy the three accelerators**, if in scope | items 3–5 again for them | 3.60 SOL |
| 12 | **Recycle** anything still inside its window | — | recovers |

Items 5 and 8 are the only two that spend money permanently, and together they
are **32.88 of the 45 SOL**. Everything before them is reversible; nothing after
them is.

**Before authorizing item 5**, all of these should be true, and none of them is
a formality:

- [ ] A checked release candidate exists at the exact deploy commit, with
      `sbf_build_diagnostics_total = 0`.
- [ ] Every buffer's bytes have been read back and hash to that candidate's ELF
      digests.
- [ ] `DCLTGMF1` has been re-measured at that commit and is under ~95% of
      1,400,000 CU.
- [ ] Blocker A is fixed and the record bodies carry observed deployment slots.
- [ ] The devnet `PythReleaseV1` row exists, minted by the adapter lane.
- [ ] The dry-run gate (§6) is green at that commit.
- [ ] The Core ephemeral authority's lifetime and destruction are decided in
      advance.

---

## 9. What this document is not

It is not a deployment, and no address in it is a dClutch address — the only
real addresses here are Pyth's, read from the public cluster. It is not devnet
evidence and it is not mainnet evidence; per `AGENTS.md`, fixtures, simulation,
local-validator execution, and devnet execution are distinct evidence levels,
and this document sits below the lowest of them because nothing in it has been
executed against a cluster at all. No deployment or frontend described here may
be called official: that requires a checked release manifest over *observed*
accounts, which Blocker B says cannot currently be produced.
