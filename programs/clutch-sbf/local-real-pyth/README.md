# Local-real Pyth signed-RPC laboratory

This is an opt-in **NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR
ONLY / NO VALUE** campaign. Its shell runner starts one explicitly selected
validator and the client admits only an explicit loopback HTTP RPC URL.

The campaign reconstructs complete Pyth receiver/router Program and ProgramData
loader bodies from the SHA-pinned captured ELFs plus decoded ProgramData keys,
deployment slots, and upgrade authority in
`svm-tests/tests/fixtures/real-pyth-local`. Raw `solana account` JSON responses
were not retained; that provenance limitation is explicit in the fixture
manifest. It initializes the
real ABIs with deterministic laboratory guardians and independently signs and
verifies two newly generated VAAs whose Wormhole-body timestamps and embedded
observations use the selected fresh boundary. The registered-feed update drives
real receiver `PostUpdate` immediately followed by Clutch
`AppendSourceArchiveV2`. The second VAA carries a different feed identity
through the real router and receiver, then proves that appending it against the
one registered SourceSpec/archive refuses atomically. The campaign also checks
wrong-Config rollback, reloads the accepted update/archive, seals the one-bucket
archive, and resolves the categorical market.

It is not a devnet price, a provider-availability test, a production source
release, current deployment evidence, or a wallet client.

With the lightweight `builder` feature, the same real-source Market/Epoch plane
also exposes daemon-only unsigned transaction construction and a shared General
V2 owner-settlement projection. The latter admits only its exact Market, Epoch,
and price scale, aggregates several filled orders under canonical
lexicographically sorted owners, requires an explicit fee row even when zero,
and retains selected owner count, buy/sell price units, fees, rounding pot, and
receipt-end count. Its 288-byte open owner bodies and prospective disposition
fields are construction results: the current General V1 campaign neither
creates those accounts nor claims their receipt authentication or execution.

The builder feature now also exposes two reusable boundaries:

- `session` creates a new explicitly named absolute session root, loopback
  ledger, secret-free public manifest, and fresh role-named key files below an
  `ephemeral-keys` directory. It never consults Solana CLI configuration or a
  default wallet, has no key loader or signer method, refuses path reuse, and
  removes a session only through an explicit marker-checked `destroy` call.
  Real-source configuration is either one SHA-256-pinned local capture or a
  credential-free HTTPS reader with an exact account-read ceiling. The same
  module constructs the selected validator's loopback-only argv, ledger,
  ports, mint identity, warp slot, and digest-bound genesis-account files
  without starting a process.
- `transaction_builder` accepts instruction bytes from their semantic owner,
  binds them to package/schema/release identity, preserves exact integer
  balance equations, and assembles unsigned blockhash-free Solana transactions.
  Its flow inventory covers SourcePlane V3, General V2 candidate construction,
  owner settlement, fees, direct Eggs, liveness, Product Series, and structured
  claims. Main-program successor envelopes use the central family allocation;
  SourcePlane and liveness keep their separately owned codecs. The resulting
  object always reports `signed=false` and `submitted=false`, and reserved SBF
  routes remain labeled `ReservedDisabled`. `build_current_workflow` refuses
  to substitute any missing flow: it requires SourcePlane V3, General V2
  candidate work, settlement, fees, direct Eggs, liveness, Series, and
  structured claims. Cursor-bearing work remains an ordered unsigned sequence;
  settlement, fees, direct Eggs, and settlement liveness are assembled into one
  atomic unsigned transaction.

Run from the repository root:

```sh
programs/clutch-sbf/scripts/run_local_real_pyth.sh
```

The runner defaults to
`.cache/agave-loopback-validator/bin/solana-test-validator` and verifies its
ignored build record against the tracked patch and toolchain pins before doing
campaign work. Build that pinned runtime once using the instructions in
`tools/agave-loopback-validator/README.md`. An explicit
`CLUTCH_LOOPBACK_TEST_VALIDATOR` or `SOLANA_TEST_VALIDATOR` is accepted only
when it resolves to that same provenance-checked cache binary.

The default `source-only-v1` mode is an exact thirteen-transaction source
campaign. Its current transcript schema is
`dragons-clutch/operator/local-real-pyth-transcript/v2`. The explicit joined
lifecycle is:

```sh
CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR=/new/empty/retained-directory \
  programs/clutch-sbf/scripts/run_local_joined_pyth_lifecycle.sh
```

The next, separately versioned two-boundary campaign is:

```sh
CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR=/new/empty/retained-directory \
  programs/clutch-sbf/scripts/run_local_multiboundary_pyth_lifecycle.sh
```

`joined-multiboundary-v1` does not reinterpret either retained one-boundary
schema. It signs two distinct registered-feed VAAs for consecutive 60-second
closing boundaries, executes both through the captured router and receiver,
and appends the resulting receiver-owned accounts in canonical order to one
two-record Source V2 page. Before the first valid append it attempts the later
boundary out of order and requires the receiver-created update, archive, and
treasury effects to roll back atomically. It then seals the complete page and
uses the same joined trade, categorical resolution, two-owner redemption, and
exact 128-atom withdrawal path. The transcript schema is
`dragons-clutch/operator/local-real-pyth-multiboundary-joined-lifecycle/v1`.
Until a clean-HEAD run is retained, this is an implemented campaign, not SBF
execution evidence; see
`docs/implementation/LOCAL_REAL_PYTH_MULTIBOUNDARY_V1.md`.
The v1 result retains both canonical archive records as exact decimal strings,
the sealed archive account key/owner/length/body SHA-256 and commitment, and
all three refused updates as the same structured evidence shape: closed
attempt kind and identity, ephemeral update address plus explicit absence,
step-linked signature/error, watched archive/receiver-treasury identities, and
equal domain-separated before/after hashes. Historical one-boundary schemas
keep their boolean rollback fields unchanged.
After both withdrawals, the multi-boundary result also reloads the complete
SupplyLedger and every authoritative Token-2022 outcome mint. Its terminal
liability object retains internal, external-ledger, aggregate, and actual mint
supplies as canonical decimal strings and is emitted only when every value is
exactly zero. Historical joined-v4 output keeps its existing field shape while
the current producer executes the same terminal zero-supply checks for that
mode.

`joined-user-lifecycle-v1` keeps Realm, Profile, policy, immutable Terms, one
collateral mint, and two ephemeral users' ordinary collateral token accounts as
disclosed genesis prerequisites. Each user begins with 64 atoms. The sealed
PriceGrid, market state PDAs, Hoard token account, outcome mints, general Epoch,
order page, reservations, candidate, verifier work, entitlement pot, and
settlement receipt must all be absent before their signed lifecycle steps.

The extended campaign initializes the real router/receiver and exactly one
registered source plane, verifies both signed VAAs, checks both atomic rollback
negatives against that registered archive, posts the correct real receiver
update adjacent to Clutch admission, and seals the source evidence without
weakening its staleness bounds. It then uploads and seals the exact typed PriceGrid and
zero-fee general clearing policy, creates the market, endows both owners, and
has the seller split a complete set. The owners place funded opposing orders
for 16 units of outcome 1. After the fixed freeze and candidate windows, the
campaign builds the canonical direct witness from the sealed order page,
submits it, runs both streaming-verifier passes plus slice verification,
selects the best valid submitted candidate, freezes and realizes entitlement,
and settles the direct slice. Only then does it resolve from the already sealed
real-Pyth source evidence, redeem both users' internal claims, and withdraw the
exact conserved 128 collateral atoms (76 to the buyer and 52 to the seller).
This 52-step producer contract is
`dragons-clutch/operator/local-real-pyth-joined-lifecycle/v4`; the historical
joined v2/v3 transcripts retain their original meanings.

Live-update maturity and freshness are checked immediately before both refused
receiver-plus-append probes and the accepted receiver-plus-append transaction.
The accepted append-time Clock and exact observation age are retained in the
result. The final Clock may be later than that live-update window because the
1,000-slot candidate window consumes already sealed source evidence; final
transcript assembly requires the Clock to remain authentic and monotone, but
does not pretend that the original update was posted again.

The historical 52-step transcript is retained at
`docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-23`. It pins
repository HEAD `4e83648479db33ca8f50798126141ab2fa262d8b`; retaining the evidence itself
is necessarily a later repository commit. The strict Operator reader consumes
only its `campaign.json`, `result.json`, and `probe-evidence.json` files. It
accepts the artifact as joined-v4 and renders the settled trade, authenticated
source resolution, redemptions, and exact terminal conservation. That
historical directory predates the public-safety gate below and its raw probe
files retain local process/path/interface metadata; it is not the publication
format for a new transcript.

The historical predecessor at
`docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-22` truthfully
reports trading as `BLOCKED / NOT SUBSTITUTED`. It remains evidence only for its
older joined-v2 schema and is never reinterpreted as the current pass. Neither
transcript is devnet, mainnet, deployment, or economic-demand evidence. The
runner never replaces a missing signed lifecycle step with mocked or
genesis-injected protocol state.

The selected validator must bind RPC, WebSocket (`RPC+1`), and faucet
listeners exclusively to `127.0.0.1`. The runner inspects the child process's
live listeners and aborts before signing if any is wildcard/non-loopback.
This is load-bearing: stock Agave 4.0.2 ignores `--bind-address` for some of
these listeners and is intentionally refused by both provenance and live-socket
checks. The Clock probe and campaign each run before/after traffic probes. RPC,
WebSocket, faucet, gossip,
and dynamic service ports are collision-checked and can be set with the
`CLUTCH_LOCAL_REAL_PYTH_*_PORT` variables. The patched outbound QUIC/UDP client
paths remain loopback-only but retain upstream's fixed validator-client port
range; the retained probe transcript therefore claims loopback isolation, not
that every client socket is inside the configurable service ranges.
Raw listener probes remain only in the private temporary campaign directory.
Before retention, `public_transcript.py` converts them to deterministic
summaries containing the exact loopback endpoints, per-endpoint observation
counts, selected-validator hash, and hashes of the ephemeral raw probes. It
then audits all five final files and refuses absolute home paths, the local
username, non-loopback literal IPs, PID/FD rows or fields, and
keypair/private-key/secret markers.

The host and SBF builds are offline and locked, reject lockfile drift, unset
compiler wrappers and Rust flags, and use a private campaign `CARGO_HOME` plus
an ignored repository-local vendored dependency source. The exact host Rust,
SBF builder/platform-tools, builder binary hash, both lock hashes, selected
validator build record, tracked validator patch/pins/provenance, and source
profile snapshot are recorded in `campaign.json`.

The standalone host crate pins Rust `1.93.1` in `rust-toolchain.toml`, and the
runner invokes that toolchain explicitly so its behavior does not depend on a
changing directory default. Install it with `rustup toolchain install 1.93.1
--profile minimal --component clippy,rustfmt` if it is not already present.
Before building, the runner requires every tracked campaign/build-input path to
have no staged or unstaged diff and records the full Dragon's Clutch `HEAD` in
the manifest; unrelated untracked user files are ignored by this source gate.

All payer/update/VAA identities are generated under the runner's private
temporary directory. No Solana CLI config or default wallet is read. The
validator and temporary secrets are removed on normal exit and signals unless
`CLUTCH_LOCAL_REAL_PYTH_KEEP_WORK=1` is explicitly set. Public, truth-labeled
`campaign.json` and `result.json` transcripts can instead be retained outside
the temporary tree with `CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR=/chosen/path`.
That directory may already exist, but the runner refuses it if any of the five
targets `campaign.json`, `result.json`, `probe-evidence.json`,
`probe-before.txt`, or `probe-after.txt` already exists; unrelated files are
left untouched. The runner builds and checks a public-safe five-file staging
set before copying any target. Raw `lsof` rows and validator logs are never
copied; only their SHA-256 provenance survives the temporary-directory cleanup.
The cheap public-safety unit gate is reproducible with:

```sh
python3 -m unittest discover \
  -s programs/clutch-sbf/local-real-pyth/tests -p 'test_*.py' -v
```

Any retained directory can be rechecked without network access using
`python3 programs/clutch-sbf/local-real-pyth/public_transcript.py check
--directory /path/to/the/five-file-transcript`.

The VAA generator is derived from `pythnet/pythnet_sdk/src/test_utils/mod.rs`
at `pyth-network/pyth-crosschain` commit
`f50a3faf9fc5a223a22889799b2f778900f186b3`. It deliberately replaces the
upstream helper's random quorum subset with guardian indices 0 through 12, so
the signed VAA is byte-reproducible for a named publish time. Upstream is
Apache-2.0; the full attribution and license are retained in
`../svm-tests/tests/fixtures/real-pyth-local/UPSTREAM_LICENSE`, and the pinned
upstream tree contains no `NOTICE` file.
