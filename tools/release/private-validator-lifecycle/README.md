# Private-validator lifecycle gate

`run.py` is the release-level localhost supervisor for one exact checked
dClutch source commit. It is not devnet evidence and it never accepts a
caller-supplied RPC URL.

The accepted mode in this revision is the one-seed founding/participant probe.
The twenty-seed terminal mode is deliberately refused until the owned-loopback
Direct producer, caller-backed payout executor, and aggregate lifecycle receipt
authenticator are all frozen in one checked source revision. The terminal steps
below describe that convergence target; they are not a claim that this revision
already completed it.

At terminal convergence, each of exactly twenty named seeds will:

1. re-authenticates `CHECKED_UPGRADE_GATE.json`, all thirteen fresh SBF links,
   and every below-4,096-byte frame report;
2. derives disposable local role keys and seven pairwise-distinct program IDs;
3. installs all seven checked ELFs as mutable Loader-v3 Program/ProgramData
   pairs, with one retained disposable authority and exact slots 1 through 7;
4. launches a fresh validator on a newly allocated loopback-only 42-port block,
   using the disposable `core-upgrade-authority` as its genesis-funded mint;
5. proves the protocol-created collateral mint, collateral wallet, and founding
   source token account are vacant before the campaign creates them;
6. executes DCLTGMF2 founding, participant admission, Direct, the eight-action
   Router/Receiver/treasury/VAA prerequisite sequence, Resolution, payout, and
   retirement through the accepted successor exterior callers;
7. keeps each stage's intent, stdout, stderr, receipt, packet journal,
   poststate, validator log, and final session under that seed's run directory;
8. kills the complete validator process group on success, refusal, signal, or
   exception.

The final `SUMMARY.json` reports the pass count and the exact arithmetic mean
of the twenty DCLTGMF2 compute measurements as numerator, denominator, floor,
and remainder. A single draw is never release evidence.

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

The Pyth boundary is explicit. After finalized Direct evidence, the supervisor
calls `local-private-validator-pyth-vaa-provision-v1` once for each canonical
journal action and once more to reauthenticate the full history and write the
exact four-field `dclutch-flagship-pyth-update-facts-v1` file. The disposable
update signer must remain vacant; the terminal Resolution caller owns Receiver
`PostUpdate` and consumes those exact facts.

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
