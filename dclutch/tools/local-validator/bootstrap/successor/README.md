# Successor immutable-infrastructure bootstrap

This standalone localhost utility prepares the exact immutable substrate for
the current multi-program successor and then drives the first market lifecycle
through a real local validator. It does not seed mutable protocol state or call
a legacy direct Resolution ABI.

The prepared plan binds seven pairwise-distinct, real SBF artifacts:

- Registry
- Core
- Claims
- Trading
- Resolution
- Custody
- RentCredit

Registry, Claims, Trading, Resolution, Custody, and Rent are represented by
immutable Loader-v3 Program accounts and exact fixed-offset ProgramData
headers followed by the exact ELFs. Core begins with the same exact ELF and a
single ephemeral upgrade authority, then must reach that immutable header by
Loader revocation before release recognition. The plan also creates distinct
`ArtifactReleaseV1` bodies, the five-role
`ExecutionReleaseSetV1`, the captured local-Pyth release body, and the expected
144-byte `ProtocolInfrastructureProfileV1` body selecting Registry and Rent.
The profile itself is not genesis-injected: its sole PDA is derived under Core
and must be created by the canonical initialization transaction.

## Evidence boundary

Only Loader accounts and the infrastructure release records required to start
the successor are prepared as genesis fixtures. Core's genesis ProgramData is
explicitly pre-init and not an accepted immutable release observation. The
supervisor executes the remaining infrastructure and market boundary as real
localhost transactions:

1. Core initialization of the sole Registry/Rent infrastructure profile.
2. Loader-v3 revocation of Core's ephemeral authority to `None`, followed by
   Registry activation of the five-role immutable release set. Activation is
   **one role per transaction**: whole-ELF hashing costs about one compute unit
   per two bytes, so admitting the real seven artifacts in a single transaction
   cannot fit under the chain maximum. A partially activated cache cannot
   decode, so no reader can consume a half-activated release set.

Loader-v3 owns authority presence with the tag at byte 12. Its real
`Some -> None` serialization leaves bytes 13..45 as inactive storage rather
than clearing the former key; the ELF still begins at byte 45. The runner pins
and verifies that exact retained-byte poststate, while Registry never exposes
inactive bytes as an authority.

3. Creation of a real Token-2022 collateral Mint and wallet, preserving raw
   `u64` atoms and treating the full `u8` decimals field as display metadata.
4. Bounded Registry `Begin -> Append -> Finalize` publication of the Realm,
   Runtime-V2 Product graph, Source material, recovery policy, and capability
   manifest. The Product root, result domain, and portfolio are compiled and
   published through one chain-derived graph state machine.
5. One same-slot pre-credit projection of the canonical Market and
   `Market+generation` lifecycle-rent PDA, followed by RentCreditV2 creation
   and finalized reacquisition.
6. Publication of a finalized address lookup table covering the Found frame,
   and submission of Found31 as a packet-safe v0 transaction. With its keys
   inline the canonical 31-account frame serialises to 1,242 raw bytes against
   Solana's 1,232-byte legacy limit — it misses by ten. Routing is table data,
   never protocol authority: only
   non-signer coordinates and the invoked Program are routed, the fee payer and
   every signer stay in the message's static key list, and the table is
   authority-owned rather than frozen so its rent stays recoverable.
7. Canonical Core Found31 creation from the post-credit snapshot.

It emits finalized transaction metadata, exact poststate account hashes, and
hostile observations for wrong infrastructure authority, pre-revocation
activation, late atomic rollback, substituted Registry refund wallet, a
substituted lifecycle credit in Found31, and a substituted Market coordinate
under attacker-chosen routing whose whole multi-instruction transaction must
roll back to a fee-only debit.

The runner creates every signing keypair in process memory, gives `prepare`
only the Core authority public key, and retains no private key on disk. The run
spec contains semantic market inputs—not account addresses or caller-authored
digests. The Rust compiler and chain-derived operators own every record digest,
PDA, instruction frame, and next publication action.

## First devnet Direct market: read-only planning authority

Before you hand a market document to the external campaign, the Direct planner
derives its capability records from authenticated facts. You give it the exact
checked deployment plan, a devnet RPC URL, the full devnet-genesis
acknowledgement, and both fee facts:

```text
--plan ABSOLUTE_JSON
--rpc-url URL
--i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
--direct-fee-basis-points U16
--direct-fee-recipient PUBKEY
```

You must supply both the fee rate and recipient. The planner deliberately has
no default, including no implied zero-fee choice, because those are operator
policy rather than facts the compiler may invent. It validates an explicit
zero basis-point value normally and still requires a nonzero recipient.

The planner is devnet-only, key-free, and read-only. It refuses loopback,
authenticates the endpoint's genesis hash, and uses one finalized
`getMultipleAccounts` observation at or above the newest checked deployment
floor. That one observation contains the Rent sysvar and the Program and
ProgramData accounts for all seven roles. The planner rechecks every Loader
link, deployment slot, upgrade authority, complete ProgramData digest, live ELF
digest, checked candidate prefix, and zero padding before it derives a market.
It does not contain a sign, send, or deploy path.

The authenticated `Market` owns the collateral decimals, so it also owns the
Direct price scale (`10^decimals`). The observed Rent sysvar owns the complete
rent minimum for the exact 256-byte Direct root. The finalized observation slot
owns the activation deadline: exactly 216,000 slots later, with checked
arithmetic. That is a provisional devnet smoke window, approximately 24 hours
at Solana's target slot time; it is an exact slot policy, not a wall-clock
promise and not a caller scalar. The capability manifest quotes the complete
root rent. Any lamports already on a vacant root are classified by root
activation rather than hidden as one-lamport "dust" in the quote.

Treat every saved plan as untrusted input. The parser bounds its byte length
and refuses duplicate keys at any depth, trailing JSON values, unknown fields,
defaulted projections, or noncanonical fields. It then reauthenticates the
checked deployment-set journal and the exact activation projection before the
first RPC read.

`devnet-market` and `graduation-market` accept exactly this surface and have no
second scalar-or-file authority path. `demo-market` is a retired local-only
fixture: it always refuses because a local fixture cannot authenticate the
permanent devnet Direct deployment, and it will not invent those facts.

## One durable terminal workflow

Use `devnet-terminal-sequence-v1` for the complete six-stage terminal workflow.
Without `--execute`, it takes bounded finalized devnet observations and saves
one unsigned durable next action before it can open your fee-payer key. With
`--execute`, it reauthenticates that saved action through the stage's semantic
owner, saves the exact signed packet and signature before the first send, and
accepts only the exact finalized transaction, balances, return data, and
account poststates. Rerun the same session and journal directory to advance.

There is no separate one-transaction lifecycle planner. In particular, no
unsigned packet becomes durable evidence merely because a caller was told to
save it. The six-stage sequence and its journal are the only public terminal
transaction owner.

## `--keypair-seed`: deterministic keys, loopback only

`run` takes an optional `--keypair-seed <64 lowercase hex>`. Default is absent,
and absent is exactly what this command did before the flag existed: one fresh
`Keypair::new()` per request.

**Why.** A different public key changes how many iterations
`find_program_address` needs to find an off-curve bump, and every extra
iteration is one `sol_create_program_address` syscall at 1,500 CU. That is the
entire source of the run-to-run compute noise `tools/gauntlet/CU_BUDGETS.md`
measures — 58,494 CU of band on `DCLTGMF1`, 79,500 on `DCLTPCB1` *within one
campaign* — and a tolerance wide enough to absorb it cannot also catch a
regression smaller than it. With a seed the band is zero.

**The derivation**, in `src/seed.rs`:

```text
index    = keys already issued for this role in this campaign, u32 little-endian
material = SHA-256( "dclutch/local-successor-bootstrap/keypair-seed/v1"
                    || 0x00 || seed[32] || 0x00 || role || 0x00 || index )
keypair  = the ed25519 keypair whose 32-byte secret seed is `material`
```

The campaign is strictly sequential — one transaction, waited to finalized,
then the next derived from it — so "the n-th key under this role" is itself a
deterministic coordinate. Every 32-byte string is a valid ed25519 secret seed,
so the derivation is total. Role names are listed in `seed::role` and are part
of the derivation: renaming one moves its keys and the compute numbers with
them, which is a budget re-pin and not a refactor.

**The safety gate is not optional.** The flag is REFUSED unless the spec's
`rpc_url` is a loopback origin, and it is refused before any key is derived. A
seed is a command-line argument: it lives in a shell history and in a checked-in
script, so every private key it derives is reproducible by anyone who can read
either. On a public cluster that hands a stranger the campaign's funded
accounts, mint authorities and upgrade authorities. `--keypair-seed` is a
TEST-ONLY affordance and this tool treats it as one.

The evidence document says which mode ran: `keypair_derivation` is
`"random-per-run"` or `"seeded-deterministic"`, and `keypair_seed_sha256`
carries the seed's digest — enough to identify which seed produced a run's
numbers, not enough to sign as it. `private_key_persisted` stays `false` either
way, because it answers a different question ("did this tool write a key to
disk") than the one a seed changes ("can anyone else produce these keys").

## What the campaign reaches: an Open Market

Measured on a real localhost validator with all seven real artifacts bound into
the release set. The campaign publishes the record graph, creates a Market
through canonical Core Found31, stages the entire projected-Custody prestate
through `DCLTPCB1` at its own generation, and then founds a *second* Market
atomically through `DCLTGMF1`.

These recorded `DCLTGMF1` observations predate the current state clean break.
Their finalized Market is the older 352-byte generation and is incompatible
with the 360-byte `DCLTCOR3` Market the current builder emits; it remains
historical evidence only.

```text
slot=1444 cu=223540   create canonical Found31 Market
slot=2021 cu=754119   create the projected-Custody founding prestate (DCLTPCB1)
slot=2630 cu=1184132  found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF1)
```

**The Market is Open.** One transaction carries five stages in one rollback
domain — Custody `LockHoardAndCloseSource`, Core Found-and-permit, Custody
`RealizeAndClose`, Claims `FoundingV5`, and Core Open **last** — at 84.6% of
Solana's 1,400,000 compute maximum:

| stage | program | CU |
|---|---|---:|
| Lock (Registry reauth 27,562; Token-2022 `TransferChecked` and `CloseAccount`) | Custody | 105,722 |
| Found and permit (Registry reauth 48,071) | Core | 414,957 |
| Realize (Registry reauth 27,562) | Custody | 87,222 |
| Claims `FoundingV5` (four Registry reauths) | Claims | 260,279 |
| Open, commit-last, plus the outer's five joins — **arithmetic**, the RPC truncated the log | Core + Trading | 315,652 |

Release-set activation is one role per transaction; the worst of the five is
Trading at 710,601 CU.

### `DCLTPCB1` is no longer heap-bound, and it never was program-side

This file used to say that the four-stage bootstrap exhausted the program heap
entering `OpenSourceCompartment` and that **no runner could fix it**, because
`RequestHeapFrame` enlarges the region the runtime grants while the stock
`solana-program-entrypoint` allocator is built with the compile-time constant
`HEAP_LENGTH = 32 * 1024`. That was true of the tree that measured it.

`9abed0c` gave Trading its own entrypoint and its own allocator, which
re-derives the grant from the instructions sysvar under agave's own
`sanitize_requested_heap_size`, and allowed exactly two routes to lift the
ceiling: `DCLTGMF1` and `DCLTPCB1`. That still did nothing, for a reason worth
recording: the adapter finds the sysvar by scanning the **top-level
instruction's own account list**, and neither route presented it. **The grant
was a wire fact, not a transaction fact.** Both routes now carry one
authenticated readonly sysvar slot in their fixed prefix, and both founding
transactions carry `ComputeBudget::RequestHeapFrame`. Frame widths moved by one:
`DCLTPCB1` 78 → 79 fixed (82 here), `DCLTGMF1` `134 + funding_count` →
`135 + funding_count` (138 here).

With the grant reaching the route, `DCLTPCB1` completes all four stages with
645,581 CU unspent. It is neither heap-bound nor compute-bound.

### Five pre-fundings, two of them exact

Nothing in the protocol funds these. Core allocates the Market and the one-shot
founding permit and Claims allocates the aggregate, the founder Position, and
the admission with `allocate` + `assign` only — **never a transfer**.

| account | requirement |
|---|---|
| Market | `lamports == rent.minimum_balance(360)` **exactly** — an over-funded Market refuses |
| one-shot permit | `lamports == rent.minimum_balance(608)` **exactly** |
| Claims aggregate | at least `rent.minimum_balance(256 + 8·claim_count)` |
| founder Position | at least `rent.minimum_balance(128 + 8·claim_count)` |
| Claims admission | at least `rent.minimum_balance(512)` |

All three Claims balances are **digest-bearing**: Core reads them at the Found
stage and folds them into the Claims request it commits to inside the permit, so
a pre-funding one lamport off does not overpay — it moves a digest and refuses
at Claims. Earlier revisions of this file said three, then four. The Found
caller-authority PDA, the fourth, needs nothing: the Market being exactly
rent-funded makes the kernel's Market rent top-up zero and the payer transfer is
skipped.

### The runner authors no digest

The founding commits to values that do not exist yet, and the campaign's
discipline survives that. The Lock and Realize receipts are produced by running
the Custody kernel's own transitions over the exact prestate bytes `DCLTPCB1`
left on chain — the `SourceFunded` projection and its normal source replay are
read back, not modelled. The permit intent and the Claims request are assembled
from the same authenticated coordinates, in the same order, that Core rebuilds
inside the Found stage. The one value that cannot be read back is the candidate
Core state the Found stage will write, whose digest the Realize receipt commits
to two stages early; every field of it is fixed by the kernel, and the encoding
is cross-checked by re-encoding the Found31 Market's own decoded state and
requiring the bytes the chain is holding.

### Final poststate

| account | owner | bytes | contents |
|---|---|---:|---|
| Market | Core | 352 | archived pre-cap generation: phase **`Open`**, readiness **`Consumed`**, no terminal receipt, derived identity, rent beneficiary the founding generation's credit |
| Claims aggregate | Claims | 288 | `256 + 8×4` for a four-outcome Product |
| founder Position | Claims | 160 | `128 + 8×4` |
| Claims admission | Claims | 512 | |
| Hoard vault | Token-2022 | 165 | the exact founding principal, `Initialized`, no delegate or close authority |
| normal Custody replay | Custody | 288 | realized in place from the 808-byte projection, one open vault, `next_revision == 1` |
| source vault, source replay, permit | — | — | **closed / consumed**, all three returned to the lifecycle credit |

### The hostile cases

Six, all executed against the real chain, all required to fail and to leave no
poststate behind.

- **`DCLTGMF1` refuses a substituted Claims request**, 33,594 CU,
  `TradingSbfError::Content` raised before Trading's first CPI. The substituted
  readonly record differs in exactly one coordinate — the **founder** whose
  Position and admission the founding mints — and is otherwise byte-identical.
  Lock, Found, Realize, and Claims all roll back; the runner requires a fee-only
  debit and all five allocated accounts still vacant.
- **`DCLTPCB1` refuses a reordered FundingState tail**, 685,198 CU. Earlier runs
  recorded this case as *not evidence*, because it refused within a few hundred
  CU of an out-of-memory death at the same place. It is attributable now: the
  honest transaction succeeds with an identical frame shape at 754,119, so the
  refusal is 68,921 CU short of anywhere the honest path ends.
- **`DCLTPCB1` refuses a well-formed but non-terminal request**, 22,860 CU,
  inside `decode_projected_request` before any CPI.
- plus the three Found31 and Registry cases this file already described.

### The way back out of a prestate that never founded

The campaign stages **two** prestates. Both run the identical four-stage ladder;
they differ in generation, in how long their founding stays satisfiable, and in
which of the two exits out of `SourceFunded` they take. The second exists to be
abandoned.

`OpenSourceCompartment` puts real collateral under a projected authority against
a Market that does not exist, and until `d43536d` nothing accepted the phase it
left behind. The forward direction is an atomic founding whose Core Found and
Open stages both refuse an expired artifact, so past `expiry_slot` that
collateral could not be moved in any direction by any route.

```text
slot=3083 cu=765807  stage a second projected-Custody prestate for the expiry abort (DCLTPCB1)
slot=3180 cu=134666  DCLTPCA1 refuses to abort a funded source before expiry     REFUSED
slot=3599 cu=148996  unwind an expired founding's funded source compartment (DCLTPCA1)
```

The refusal is the half that matters: while the founding is still satisfiable
the authority over funded principal may not be destroyed. It refuses with
`CustodySbfError::Expiry`, rolls the whole multi-instruction transaction back to
a fee-only debit, and the runner then re-authenticates the entire prestate to
prove it moved nothing. The honest abort, 419 slots later, returns exactly the
principal to the party that supplied it, closes the source vault, the source
replay, the empty Hoard vault, and the projection, and raises the lifecycle
credit by exactly those four rents.

The abort lane's expiry is squeezed from both sides — `initialize` refuses once
`current_slot > expiry_slot`, so it must outlast staging, and every slot past
that is dead waiting. The runner does not trust the arithmetic: it checks the
remaining margin before staging and waits for the real slot afterwards.

### What the frame forced

- The founding runs at **its own generation**: every projected-Custody stage
  asserts the inverse of a live Market, so the Found31 Market cannot be reused.
- The **principal supplier is not the rent payer**, and the beneficiary is named
  once — Custody requires the principal's owner to sign while non-writable and
  the payer to be writable, and privileges are per key. The same key is the
  founding artifact's `beneficiary`, the Lock's `refund_owner`, the owner of the
  Token-2022 source wallet, and the lifecycle credit's refund wallet.
- **No account in the `DCLTGMF1` frame is a transaction-level signer**, so the
  fee payer is the one key that appears nowhere in it. **65 distinct keys**,
  eleven of them writable, against agave's 128-account lock limit.
- Privileges are **unioned per key** before sending. The Market is the clearest
  case: Custody's Lock stage requires it non-writable and vacant, and Core's
  Found stage creates it.
- `context_digest = sha256(b"dclutch:projected-hoard-context:v1" || context)`
  while `funding_source_context` is that context **undigested**; both are
  caller-PDA seed inputs, so a wrong one produces an address for which no
  signature exists.
- The Market identity the campaign carries has a **placeholder `market_id`**,
  because `market_id` is not one of the nine Market-address seeds. That is
  harmless until a founding has to commit to the digest of the Core state the
  Found stage will write.

Two earlier defects on this path were fixed at their semantic owners: the host
Found/RentV2 projections refused the real System Program (`c25de27`), and the
capability-root selection was a SHA-256 fixed point and so unsatisfiable for
every well-formed artifact (`386f254`). The full decomposition, the per-role
activation table, the artifact digests, and the complete transcript are in
`docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md`.

## Sponsored Pyth push exterior

You can capture the public devnet SOL/USD sponsored account without a Hermes
token, a Pyth Terminal subscription, or a caller-owned Receiver update. The
command reads the fixed onchain account directly, authenticates the active
Resolution role through the Market's checked release set, and derives the
candidate, head, certificate, and receipt addresses from finalized bytes.

Start with a read-only preflight. The output path must not already exist:

```sh
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml -- \
  devnet-sponsored-push-v1 \
  --rpc-url https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --input /absolute/path/sponsored-market.json \
  --output /absolute/path/capture-preflight.json \
  --action capture \
  --signer YOUR_SPONSOR_PUBKEY
```

The input is one routing document. Each `raw`/`staging` pair names an existing
finalized Registry record and its vacant staging account:

```json
{
  "format": "dclutch-sponsored-push-exterior-input-v1",
  "generation": 1,
  "terminalSequence": 0,
  "releaseSet": "64_lowercase_hex_characters",
  "accounts": {
    "registryProgram": "PUBKEY",
    "activationCache": "PUBKEY",
    "coreProgram": "PUBKEY",
    "resolutionProgram": "PUBKEY",
    "resolutionProgramdata": "PUBKEY",
    "market": "PUBKEY",
    "sourceState": "PUBKEY",
    "sourceMaterial": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "sourceSpec": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "providerRelease": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "adapterConfig": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "window": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "statistic": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "sponsoredRelease": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "product": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "resultDomain": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "portfolio": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "capabilityManifest": { "raw": "PUBKEY", "staging": "PUBKEY" },
    "failureFunding": "PUBKEY",
    "priceAccount": "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE",
    "receiverProgram": "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ",
    "receiverProgramdata": "96QrNCjmh32H9quY9DX4NEH81nECVsbkATBDZeoVbvLV",
    "pushOracleProgram": "pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT",
    "pushOracleProgramdata": "8xAeURaAWExxyHUXJSgjsg5r96Ydr3G4cek2if7imQmz",
    "receiverConfig": "DaWUKXCyXsnzcvLUyeJRWou8KTn7XtadgTsdhJ6RHS7b",
    "lookupTable": "FROZEN_ADDRESS_LOOKUP_TABLE_PUBKEY"
  }
}
```

`failureFunding` is the active Resolution-owned `FundingLedgerV2` left by the
V6 controller-funding split after Market Open. It is not the transient
controller-funding checkpoint: Open already authenticated and closed that
checkpoint after Custody staging. Sponsored capture and settlement never
recreate, stage, or consume pre-Market funding; the head-vacant failure action
authenticates and debits the exact active Resolution ledger that the opened
Market selected.

The lookup table is routing, never authority. It must already be active,
finalized, and frozen, and it must contain enough of this Market's sponsored
frame to keep every action below Solana's 1,232-byte packet ceiling. The caller
persists its complete account body and compiles the canonical v0 message from
those exact bytes; a table substitution, mutable authority, later extension,
or packet above the ceiling refuses. The 32-account Settle frame is 1,277 bytes
as a legacy packet and therefore cannot be honestly submitted without routing.

To submit after reviewing the preflight, rerun the exact command with the same
output path and add `--execute --signer-keypair /absolute/path/keypair.json`.
The report advances atomically through `planned`, `prepared`, `submitted`, and
`finalized`. `prepared` contains the exact signed packet, signature, packet
digest, and last-valid block height before the first send. A restart from
`prepared` may send only those same bytes; a restart from `submitted` is
poll-only and cannot rebuild or submit a second signature. Once the report is
`prepared`, the keypair is no longer required for restart. If the exact packet
expires without a finalized status, the command preserves that journal and
requires a new action under a new output path.

One create-new lock covers the report's entire read, transition, send, and poll
run. A normal exit removes it. A crash leaves it fail-closed; confirm that no
process still owns the report before you manually remove the named stale lock.

Use
`terminalSequence: 1` for `settle` or `commit-failure`, and zero for the other
actions. `settle` reads the selected candidate from the canonical head.
`close-candidate` additionally requires `--candidate PUBKEY`. Cleanup is
permissionless, but its transaction payer must be different from every account
in the cleanup instruction; in particular, do not reuse the captured rent
beneficiary as that transaction's payer.

The report format is `dclutch-sponsored-push-exterior-report-v2`. It retains
the reviewed instruction and prestate, the durable signed packet, finalized
signature/fee/compute/log evidence, and the writable poststates in that one
path. The keypair file and secret bytes never enter the report.

## General and Series: one durable family caller

`local-private-validator-general-hot-campaign-v1` drives all seven authored
General actions against the real accelerator ELF on a local validator, and
`local-private-validator-series-hot-campaign-v1` is its Series sibling. They
share one journal ladder — `planned`, `prepared`, `submitted`, `finalized`,
each phase renamed into place — because what the two families have in common
is not their semantics but their transaction discipline.

The campaign runs in **two phases**, and the first one deliberately opens no
socket and no key file. Its output is a set of genesis account fixtures, and
those have to exist before the validator they are loaded into does.

### Phase 1: prepare the fixtures

```sh
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml -- \
  local-private-validator-general-hot-campaign-v1 \
  --accelerator ACCELERATOR_PROGRAM_PUBKEY \
  --caller CALLER_PROGRAM_PUBKEY \
  --account-dir /absolute/empty/dir/accounts \
  --journal-dir /absolute/empty/dir/journals \
  --evidence /absolute/path/evidence.json
```

This writes one `<address>.json` per account the accelerator reads and one
planned journal per action. The addresses are **derived**, not fresh keypairs:
the two phases run in different processes and must agree on them without
passing a key, and nothing ever signs as these accounts.

Why fixtures at all — a host cannot write into an account it does not own.
These accounts hold data owned by the caller program, only the owning program
could write them, and the readonly caller has no instruction that would. So
they enter at genesis, exactly as the Loader pairs do.

### Phase 2: start the validator and execute

```sh
solana-test-validator --ledger /absolute/new/ledger --reset \
  --rpc-port 21400 --faucet-port 21402 --gossip-port 21403 \
  --dynamic-port-range 21410-21441 --ticks-per-slot 16 \
  --account-dir /absolute/empty/dir/accounts \
  --bpf-program ACCELERATOR_PROGRAM_PUBKEY /abs/dclutch_general_accelerator_sbf.so \
  --bpf-program CALLER_PROGRAM_PUBKEY /abs/dclutch_general_accelerator_test_caller_sbf.so
```

Build those two ELFs with `cargo build-sbf --manifest-path
programs/dclutch-general-accelerator-sbf/Cargo.toml --sbf-out-dir DIR` and the
same for `test-programs/general-caller`; their program ids are the pubkeys of
the keypairs that build emits. Pick an RPC port block that is yours — the
whole block is `BASE`, `BASE+2`, `BASE+3`, `BASE+10..BASE+41`, and lanes share
a machine.

Then rerun phase 1's command with the three flags that make it write:

```sh
  --execute --rpc-url http://127.0.0.1:21400 --payer-keypair /absolute/payer.json
```

### What it reaches

Measured at runtime width 1, **eleven finalized transactions, all accepted**:

| step | action | ACK | CU | legacy packet |
|---|---|---|---:|---:|
| 0 | Consider | accepted | 34,825 | 863 |
| 1 | Freeze | accepted | 31,371 | 797 |
| 2 | InitializeSettlement | accepted | 60,466 | 920 |
| 3–5 | Collect × 3 rows | accepted | 55,703 / 56,908 / 56,885 | 900 |
| 6 | Materialize | accepted | 51,884 | 866 |
| 7–9 | Distribute × 3 rows | accepted | 55,654 / 56,859 / 56,890 | 900 |
| 10 | Close | accepted | 60,046 | 885 |

**Seven actions, eleven steps.** `Collect` and `Distribute` each consume the
three settlement manifest rows, and `Materialize` sits between them because the
collected inventory has to exist as a complete set before anything is
distributed. The settlement half is a *chain*: every step reads the cursor the
previous one produced, and the whole chain is derived natively — in
`general_settlement_fixture.rs` — before a single transaction is signed. A
broken chain therefore fails on the host, which is where it should.

The same eleven steps are accepted at `--outcome-count 4` (CU 31,516–61,379,
packets unchanged at 797–920 bytes, because the register bank travels in
scratch pages rather than in the packet). The fixture is not width-one.

**The width is not a preference.** Six of the seven serialise past Solana's
1,232-byte legacy ceiling at N=258, so a campaign claiming that width would be
recording routes no validator would accept. N=1 is where the packet clause
holds, and the caller refuses N=258 explicitly rather than discovering it.

**Nothing in the settlement frame is a literal.** The verifier cursor, the
verified-candidate certificate and the settlement manifests are outputs of a
real collection half: a batch opened on a live root, three signed portfolio
orders admitted into it, the batch closed, a candidate addressed by its own
digest, a submission funded, and every row put through
`verify_candidate_row_v1` — the protocol's own verification verb.

**If you build a General Hot frame yourself, write all the bank identities.**
Five of them are route-dependent, and for `SOURCE_VAULT_CONTEXT` and
`DESTINATION_VAULT_CONTEXT` **zero is what the *enabled* route requires**. An
unwritten register does not read as absent; it reads as live. That asymmetry is
why a partially-filled bank looks like it is almost working — the actions whose
zero defaults happen to be correct pass, and the rest refuse.

### What it does not reach

**This is the evaluation half of the family.** The accelerator owns no account,
signs nothing and performs no CPI. The commit half — Trading's
`process_hot_execution_v3` writing the capability root and returning a
`DCLTHAK3` ack — additionally needs a founded Market whose capability manifest
selects General, and whose capability root exists. Both now have drivers:
`local-private-validator-market-v1` with `DCLUTCH_MARKET_CAPABILITY=general`
compiles the selected market input, and
`local-private-validator-general-capability-activation-v1` creates the root.
`general_market_selection_requirements_v1` states the six facts such a Market
must carry.

## General capability activation

```sh
dclutch-local-successor-bootstrap \
  local-private-validator-general-capability-activation-v1 \
  --rpc-url http://127.0.0.1:PORT/ \
  --plan ABSOLUTE_PLAN_JSON --campaign-report ABSOLUTE_FOUNDING_JSON \
  --payer-keypair ABSOLUTE_DISPOSABLE_JSON \
  --output ABSOLUTE_NEW_JSON [--execute]
```

One Core-signed transaction creates `CapabilityRootHeaderV1 || GeneralRootV2` at
the manifest-selected root PDA, moving the funding ledger's parked Rent quote
into it. Without `--execute` it plans, writes the derived coordinates, and
signs nothing. A live Trading-owned root reports `already-active`.

The campaign report supplies two ROUTING coordinates — the Market address and
the Trading funding-ledger address. Everything semantic is re-derived from
chain: the manifest from the Market's own identity, the General entry found in
it by kind, the ProgramSet from that entry's `release_id` (which must
authenticate as `SettlementWithActivation` — a seven-entry General release
refuses here, because nothing founded on it could ever create a root), the
activation descriptor selected out of that set, and the account-profile and
effect records off that descriptor. The poststate is read with General's own
decoder and checked against `GeneralRootV2::active`,
`general_root_creation_tail_v2`, and `FundingLedgerV2::activate_in_place`.

Loopback only. The Direct sibling, `devnet-direct-capability-activation-v1`,
is the same route on an acknowledged devnet endpoint.

**Series compiles its wire and refuses.** See that command's own refusal text
for the exact reason; the short version is that the common authenticated Shadow
callback Trading would need is not committed, so no Series action has a
dispatched Hot route to run through at all.

### Resuming

The journal on disk is read before anything is written. Past `prepared` a run
may only reauthenticate the persisted bytes and send them again; a `finalized`
journal is polled rather than resubmitted. Rerunning a completed campaign
produces byte-identical signatures and slots and submits nothing. A journal
whose action, width, caller or family-request digest differs from the run in
hand is refused rather than resumed.

## Series: the first executed Found

`local-private-validator-series-consume-v1` submits one `series_consume`
transaction — the only Series route this tree dispatches — to a live local
validator. It is a **write path**: the transaction *is* the Core Found.

The campaign is **not** authored here. Its fixture is roughly 1,250 lines in
`programs/dclutch-core-sbf/tests/found_program_test.rs`, and porting it would
give one campaign two authors who could silently disagree. Instead that file
gained one `#[ignore]`-gated emitter which builds the campaign exactly as every
other Series test does, starts the genesis it would have run against, and reads
every account the instruction names back out of the banks client.

### Emit the bundle

```sh
SBF_OUT_DIR=/abs/sbf DCLUTCH_SERIES_CAMPAIGN_DIR=/abs/campaign \
  cargo test --manifest-path programs/dclutch-core-sbf/Cargo.toml \
  --test found_program_test -- --ignored emit_series_consume_validator_campaign
```

Build the four ELFs it reads first (`dclutch-core-sbf`, `dclutch-registry-sbf`,
`dclutch-rent-sbf`, and `test-programs/series-consume-caller`) with
`cargo build-sbf --sbf-out-dir`. Output: 61 genesis account files under
`accounts/`, and `campaign.json`.

### Start the validator and submit

```sh
solana-test-validator --ledger /abs/new/ledger --reset \
  --rpc-port 21600 --faucet-port 21602 --gossip-port 21603 \
  --dynamic-port-range 21610-21641 \
  --account-dir /abs/campaign/accounts
```

**No `--bpf-program` flag.** The six loader-v3 Program *and* ProgramData
accounts are in the bundle with their real bytes and executable bits, because
their deployment slot flows into the release-set digest and therefore into the
Market PDA — deploy-then-derive is circular with genesis here.

**Use the default tick rate.** The occurrence's retry window is 10,000 slots
measured from slot zero. At the stock 64 ticks per slot that is about 66
minutes; at 16 it is about 16, and you will be debugging against a timer.

```sh
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml -- \
  local-private-validator-series-consume-v1 \
  --campaign /abs/campaign/campaign.json --rpc-url http://127.0.0.1:21600 \
  --payer-keypair /abs/payer.json --journal /abs/campaign/journal.json --execute
```

Without `--execute` it preflights: it proves the lookup table is populated, the
invoked program is executable, and the Market is still vacant — each of which
would otherwise surface as an opaque `Custom(...)` deep inside Core.

### Measured

| | |
|---|---:|
| outcomes | 258 |
| account metas | 62 (61 unique) |
| instruction data | 656 bytes |
| routed wire | 1,037 bytes (v0 + ALT) |
| compute units | 624,620 |

Legacy routing is not tight here, it is impossible: 61 keys is 1,952 bytes of
addresses against a 1,232-byte packet. The compute budget comes from
`bounded_instructions`, which already asks for 1,400,000 — a real validator's
200,000 default would refuse.

**A write path's acknowledgment is the state it committed**, not a returned
buffer, so the consumer authenticates the Market being Core-owned and written
and the founding permit holding its exact expected balance. Rerunning a
finished campaign is a no-op; that check necessarily precedes the vacant-Market
preflight, because a succeeded Found is exactly what makes the Market
non-vacant.

### Hostiles

`--expect-refusal CODE` skips the vacant-Market precondition and requires the
transaction to fail with exactly that code. It **skips preflight on purpose**:
a hostile rejected by simulation proves what a simulator thinks, not what the
chain did. The double-consume runs as:

```sh
  ... --journal /abs/campaign/hostile.json --expect-refusal 12293 --execute
```

and commits in its own slot carrying `{"Custom":12293}` — `CoreSbfError::Market`
— leaving the Found it tried to overwrite byte-unchanged. A hostile that
*succeeds* is reported as the loudest possible failure.

### Debt

The proper port into this tool alongside the General arm is deliberately
deferred. The emitter/consumer split keeps the campaign authored once but
living in two places; a future lane wanting Series in the same shape as General
should close that.
