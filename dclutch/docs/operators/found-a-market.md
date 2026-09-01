# Found a market on your own validator

You end this walkthrough holding a chain of your own: seven dClutch programs
deployed, protocol infrastructure published and activated, one market
founded and open, and one participant admitted to it — every step a real
transaction submitted to a real validator at real limits. Then the run tears
the chain down.

It takes about twenty-five minutes and about three and a half gigabytes of
disk.

## What you need

```sh
solana --version && solana-test-validator --version && python3 --version
```

```
solana-cli 4.0.2 (src:549805f3; feat:6ff76655, client:Agave)
solana-test-validator 4.0.2 (src:549805f3; feat:6ff76655, client:Agave)
Python 3.14.6
```

You also need two things that are not tools: a **clean checkout at one exact
commit**, and a **checked release root**.

## The one input you cannot build offline

A checked release root is a directory holding `CHECKED_UPGRADE_GATE.json`
and the thirteen freshly built program artifacts it attests. It is built by
`tools/release/checked-release-candidate.sh` — which requires
`--predecessor-profile`, a 144-byte infrastructure profile account dumped
from the chain being succeeded. Its own help says why:

> the one input that cannot be built from source: a succession is not a
> function of the successor alone, so the predecessor's own two binding ids
> — which the ceremony copies into the profile it commits — have to be read
> from the chain being succeeded.

So the release root is where this act begins, and it is not reachable from a
cold machine with no network. There is no committed fixture for the
predecessor profile in this tree. Either you already have a release root, or
you dump that account from the cluster you are succeeding first.

Everything below uses an existing one. Read its pins, because they decide
which commit you must check out:

```sh
jq -r '{schema, source_revision, source_tree_sha256}' <RELEASE_ROOT>/CHECKED_UPGRADE_GATE.json
shasum -a 256 <RELEASE_ROOT>/CHECKED_UPGRADE_GATE.json
```

```json
{
  "schema": "dclutch-checked-upgrade-gate-v1",
  "source_revision": "45e7adc07f75b0553f4251335c07b2d1f0b607e6",
  "source_tree_sha256": "644db7f1728881918e4f22dffbfc526ae8f1c18da455e4ee70e2a052fbf20f67"
}
```
```
42882616f1d5a0560ab8b4dd54e8ceedd290049c8661332cc9aa4cdab70db11d
```

## Get a clean checkout at that commit

```sh
git worktree add --detach ~/work/src-45e7adc0 45e7adc07f75b0553f4251335c07b2d1f0b607e6
```

```
Preparing worktree (detached HEAD 45e7adc0)
HEAD is now at 45e7adc0 goal: all-targets tells no lies
```

A worktree, not your working checkout. The gate binds the commit and the
tree digest, and rechecks the source bytes before and after the run — so a
tree with one edited file is a tree whose evidence would name a commit that
is not what executed.

## Check the source contract

Before spending a build or a validator launch, run the offline model. It
performs no build, no RPC, no key read, and no validator action.

```sh
python3 tools/release/private-validator-lifecycle/preflight.py \
  --repo ~/work/src-45e7adc0 --through participant \
  --output ~/work/PREFLIGHT.json
```

```json
{
  "status": "accepted",
  "through": "participant",
  "validator_started": false,
  "rpc_used": false,
  "keys_read": false,
  "evidence_level": "offline-clean-committed-source-contract-only"
}
```

### If the repository is not clean

**Remedy: commit your work, or point `--repo` at a fresh worktree.**

```
private-lifecycle-preflight: REFUSED: repository is not clean; offline
preflight requires one exact committed snapshot
```

Never resolve this by stashing. A stash mutates the tree every other process
on the machine is reading.

## Run it

```sh
python3 tools/release/private-validator-lifecycle/run.py \
  --repo ~/work/src-45e7adc0 \
  --release-root <RELEASE_ROOT> \
  --expected-release-gate-sha256 42882616f1d5a0560ab8b4dd54e8ceedd290049c8661332cc9aa4cdab70db11d \
  --expected-release-source-revision 45e7adc07f75b0553f4251335c07b2d1f0b607e6 \
  --expected-release-source-tree-sha256 644db7f1728881918e4f22dffbfc526ae8f1c18da455e4ee70e2a052fbf20f67 \
  --validator "$(command -v solana-test-validator)" \
  --solana "$(command -v solana)" \
  --work ~/work/found-probe \
  --through participant --seeds 1
```

The three `--expected-*` pins are optional and worth passing. Without them
the run trusts whatever gate is at `--release-root`; with them it refuses a
release root that is not the one you meant.

There is no `--rpc-url` here, and there is no `--i-mean-devnet`. This
supervisor never accepts a caller-supplied endpoint: it launches its own
validator on a free loopback port block and talks to that. A loopback origin
is admitted with no ceremony. The acknowledgement flag exists for the
drivers that *do* take an endpoint, where it takes the cluster's genesis
hash spelled out in full rather than a boolean, so that a command line
copied to another cluster stops being true. Mainnet is refused
unconditionally and no flag admits it — including on a loopback origin,
because a loopback port can be a tunnel.

### Three refusals you will meet if you improvise

**Remedy: pass `--seeds 1`.**

```
private-validator-lifecycle: REFUSED: the founding/participant development
probe requires exactly one named seed
```

**Remedy: use `--through participant` or `--through full-probe`.**

```
private-validator-lifecycle: REFUSED: twenty-seed private-validator release
evidence is not accepted in this revision; missing semantic owners: exact
seventeen-case resumable chaos session
```

Twenty-seed mode is the release gate, and it stays refused until the work it
would cite exists. A mode that cannot yet be honest refuses rather than
running and reporting less than it claims.

**Remedy: name a directory that does not exist yet.**

```
private-validator-lifecycle: REFUSED: --work must be a fresh absolute
directory with an existing parent
```

The runner will not overwrite prior evidence.

## What it does

The supervisor builds the host binary itself into the work root, so an
ambient or stale executable cannot drive the run. Then, per seed:

| stage | |
|---|---|
| `01-prepare-mutable` | derive disposable role keys and seven distinct program ids from the gate |
| `02-authenticate-mutable` | re-authenticate that plan offline |
| — | launch `solana-test-validator` on a free 42-port loopback block |
| `03-local-test-bankroll` | disposable funding |
| `04-administration` | publication, profile initialization, activation |
| `05-market-input` | compile one canonical market against the live chain |
| `06-founding` | **the market is founded and opened** |
| `07-participant-bankroll` | fixture liquidity |
| `08-participant` | admit one participant |

Observed timings on one run: offline preflight at 17:30:49, host binary
built by 17:33:50, validator answering by 17:34:04, founding evidence
written at 17:52:53.

## What you get

```json
{
  "schema": "dclutch-private-validator-participant-probe-summary-v1",
  "through": "participant",
  "source_revision": "45e7adc07f75b0553f4251335c07b2d1f0b607e6",
  "status": "passed",
  "finalized_stages": ["founding", "participant"]
}
```

The founding evidence records `"cluster": "loopback"`, `"mode": "execute"`,
its own genesis hash (`DgVLziCYNS7wmjj565dZ3XanmFvgXyzaFAeBwcRkJ5HF` on this
run — yours differs, the chain is new), and the addresses it created:

```
collateral_mint   2oC9JMSj18W5zF1JoHLvMJtogXWhvo127jWiPWsuJ8wn
realm_record      EDCLCi87SNqBMEGmV2hRPnXyo6iy2YLdvvY1VLJxLF7k
open_market       3UugcUQt7ViiZ3GRurWHZ8vAoL9dwiTWG5XjJ8AqoAWS
found31_market    25Q2EJr1nEu2x4KYuX8rVw7o1Vj1EpkkssVLYgTDGNdg
abort_market      37mVw39i3J7pYfGPvJqQzppeVQFzmMQBvxJtXpY3utTh
```

And what each founding transaction actually cost, measured by the chain:

```
founding-core-funding-accept            189,836
founding-core-funding-create            322,224
founding-dcltcfq1                       587,761
founding-dcltpcb2                       876,544
founding-dcltgmf3                     1,069,561
founding-resolution-funding-activate    283,274
```

Solana's per-transaction maximum is 1,400,000 compute units. The atomic
founding, `DCLTGMF3`, sits at 76% of it. That is the number to watch: it is
one transaction and there is no headroom to buy.

The run keeps every role keypair under `runs/seed-01/mutable/keys/` and
records `"private_key_persisted": true` — these are disposable keys for a
chain that is about to be destroyed, and the evidence says so out loud
rather than leaving you to assume it.

## What it costs

```
3.0G    runs/seed-01/ledger
3.5G    (work root)
```

The ledger is the expensive part, at roughly half a gigabyte per thousand
slots. It is retained rather than purged because the drivers re-verify every
earlier stage from transaction history on every invocation: a purge that
lands mid-sequence does not slow a campaign down, it ends it, and there is
no resume. Budget the disk, and reap work roots when you are done with them.

## The chain goes away

When the run finishes it kills the whole validator process group — on
success, refusal, signal, or exception. A watchdog enforces this even if the
supervisor is `SIGKILL`ed, because a finished campaign once left a validator
with `PPID 1` holding the one port every run needed, on a chain nobody could
use because the founder key had died with the supervisor's memory.

So the market you founded is gone when the command returns. To keep it —
to trade on it, resolve it, or hand it to another driver — pass
`--hold-after-participant`. The supervisor then writes a mode-0600
`runs/seed-01/participant-handoff.json` and stops *itself* with `SIGSTOP`,
leaving the validator and watchdog alive; you send `SIGCONT` when you are
done. Keep that stopped process alive, or restart `solana-test-validator`
against `runs/seed-01/ledger` directly — the account state survives a
restart. That path is not exercised here.
