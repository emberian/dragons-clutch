# Private-validator lifecycle gate

`run.py` is the release-level localhost supervisor for one exact checked
dClutch source commit. It is not devnet evidence and it never accepts a
caller-supplied RPC URL.

The accepted executable mode in this revision is the one-seed
founding/participant probe. The post-Direct Pyth, Resolution, payout,
retirement, finalized-capture, activity-manifest, provider-closure, session,
aggregate-receipt, and dossier surfaces are implemented. The one-seed
`full-probe` and twenty-seed `full` modes remain deliberately refused until the
owned-loopback Direct producer exports one finalized terminal semantic owner
and its canonical nonzero-claim payout schedule. The terminal steps below are
the convergence target; they are not a claim that a full lifecycle has run.

At terminal convergence, each of exactly twenty named seeds will:

1. re-authenticates `CHECKED_UPGRADE_GATE.json`, all thirteen fresh SBF links,
   and every below-4,096-byte frame report;
2. derives disposable local role keys and seven pairwise-distinct program IDs;
3. installs all seven checked ELFs as mutable Loader-v3 Program/ProgramData
   pairs, with one retained disposable authority and exact slots 1 through 7,
   plus exact immutable slot-zero Pyth Receiver and Router pairs from the same
   authenticated eighteen-account genesis directory;
4. launches a fresh validator on a newly allocated loopback-only 42-port block,
   using the disposable `core-upgrade-authority` as its genesis-funded mint;
5. proves the protocol-created collateral mint, collateral wallet, and founding
   source token account are vacant before the campaign creates them;
6. executes DCLTGMF2 founding, participant admission, Direct, the eight-action
   Router/Receiver/treasury/VAA prerequisite sequence, Resolution, payout, and
   retirement through the accepted successor exterior callers;
7. derives the six public activity stages and the distinct eight-stage private
   completion from their semantic owners, captures all transactions and final
   accounts at one finalized RPC boundary, and re-authenticates the nine Loader
   pairs from raw captured bytes;
8. writes one provider closure, aggregate receipt, and no-clobber reconciled
   dossier under that seed's evidence root;
9. keeps each stage's intent, stdout, stderr, receipt, packet journal,
   poststate, validator log, and final session under that seed's run directory;
10. kills the complete validator process group on success, refusal, signal, or
    exception.

The final `SUMMARY.json` reports the pass count and exact arithmetic mean of
every named transaction's compute measurement across twenty seeds as
numerator, denominator, floor, and remainder. A single draw is only the
explicit `full-probe` integration gate, never final release evidence.

Once the terminal mode is accepted, run it on hbox from a clean clone of the
admitted commit and a fresh checked release root:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
  python3 tools/release/private-validator-lifecycle/run.py \
  --repo /tank/dregg-build/dclutch-private-lifecycle-source \
  --release-root /tank/dregg-build/dclutch-release \
  --validator /usr/local/bin/solana-test-validator \
  --solana /usr/local/bin/solana \
  --work /tank/dregg-build/dclutch-private-lifecycle-REV
```

`--repo` must be a clean clone (or detached clean worktree) at the gate's
source commit. The supervisor builds the successor host binary itself into the
fresh work root, so an ambient or stale bootstrap executable cannot drive the
evidence run.

The supervisor refuses before validator launch if the source is dirty, the
gate is stale, any shipped link is missing, or an exterior command is absent.
It never falls back to the retired immutable journey, because that journey
still records Direct, payout, and final retirement as gaps and therefore
cannot prove this lifecycle.

The Pyth boundary is explicit and requires no hosted Pyth API credential. The
fresh validator loads the pinned Receiver and Router ELF fixtures as truly
immutable Loader-v3 accounts (`deploymentSlot = 0`, null upgrade authority),
and the provider closure independently decodes their finalized Program and
ProgramData bytes. After finalized Direct evidence, the supervisor
calls `local-private-validator-pyth-vaa-provision-v1` once for each canonical
journal action and once more to reauthenticate the full history and write the
exact four-field `dclutch-flagship-pyth-update-facts-v1` file. The disposable
update signer must remain vacant; the terminal Resolution caller owns Receiver
`PostUpdate` and consumes those exact facts.

The final evidence path uses the callable successor commands below. They are
semantic-owner producers, not licenses to hand-author JSON:

```text
local-private-validator-pyth-provider-closure-v1
local-private-validator-activity-stage-completion-v1
local-private-validator-activity-manifest-v1
local-private-validator-finalized-activity-capture-v1
local-private-validator-lifecycle-session-v1
local-private-validator-lifecycle-receipt-v1
```

The dossier is the last write. Its reconciler reopens the exact aggregate
receipt by caller-supplied SHA-256 and creates a new output without replacing
an existing file or following a symlink.

Today, the caller-backed development probe stops after the real founding and
participant transactions. The local market mints an explicit extra 100,000,000
raw collateral atoms into the `direct-buyer` fixture account owned by the
`participant`, removes mint authority, and records that separate supply before
the participant caller transfers and approves the exact amount:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
  python3 tools/release/private-validator-lifecycle/run.py \
  --repo /tank/dregg-build/dclutch-private-lifecycle-source \
  --release-root /tank/dregg-build/dclutch-release \
  --validator /usr/local/bin/solana-test-validator \
  --solana /usr/local/bin/solana \
  --work /tank/dregg-build/dclutch-private-participant-probe-REV \
  --through participant --seeds 1
```

That mode emits a distinct participant-probe summary and can never be mistaken
for the 20-seed full-lifecycle gate. The development market binds the exact
integer policy to the owned `founding-source-funder` identity: seller fee atoms
are `floor(seller gross collateral atoms * 50 / 10,000)`, buyer fee atoms are
`floor(buyer gross collateral atoms * 50 / 10,000)`, and the recipient gets
their exact sum. The realized Direct stage remains the fee semantic owner's
evidence.

When the callable Direct exterior must consume the same authenticated local
state before it is integrated into this supervisor, add
`--hold-after-participant`. After finalized participant admission the
supervisor writes `runs/seed-01/RESULT.json` and a create-new, mode-0600
`runs/seed-01/participant-handoff.json`, then stops itself with `SIGSTOP` while
leaving its exact validator child and watchdog alive. The handoff receipt owns
only process control: source and checked-gate digests, loopback RPC, validator
PID, canonical plan/market/founding/participant/key-directory paths, and the
participant evidence digest. It never copies key bytes or claims Direct
completion.

The external Direct driver must wait until the supervisor process is visibly
stopped before using those paths. After it has durably finalized or refused,
send `SIGCONT` to the supervisor PID. Before teardown, the resumed supervisor
reopens the exact receipt, checks its mode and bytes, proves the original
`Popen` child/process-group identity is still live, and requires the same
loopback RPC to report healthy. Any substitution or dead child refuses and the
ordinary `finally` boundary still terminates the complete validator group.
