# Local-real Pyth signed-RPC laboratory

This is an opt-in **NON-PRODUCTION / SYNTHETIC OBSERVATION / LOCAL VALIDATOR
ONLY / NO VALUE** campaign. Its shell runner starts one explicitly selected
validator and the client admits only an explicit loopback HTTP RPC URL.

The campaign loads the reconstructed exact complete SHA-pinned Pyth
receiver/router Program and ProgramData bodies from the captured ELFs plus the decoded
ProgramData keys, deployment slots, and upgrade authority in
`svm-tests/tests/fixtures/real-pyth-local`. These are reproducible complete
loader-account bodies, not retained raw `solana account` JSON responses; that
provenance limitation is explicit in the fixture manifest. It initializes the
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

Run from the repository root:

```sh
CLUTCH_LOOPBACK_TEST_VALIDATOR=/absolute/path/to/solana-test-validator \
  programs/clutch-sbf/scripts/run_local_real_pyth.sh
```

The default `source-only-v1` mode is an exact thirteen-transaction source
campaign. Its current transcript schema is
`dragons-clutch/operator/local-real-pyth-transcript/v2`. The explicit joined
lifecycle is:

```sh
CLUTCH_LOOPBACK_TEST_VALIDATOR=/absolute/path/to/solana-test-validator \
  CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR=/new/empty/retained-directory \
  programs/clutch-sbf/scripts/run_local_joined_pyth_lifecycle.sh
```

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

The current public-safe 52-step transcript is retained at
`docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-23`. It pins
repository HEAD `4e83648479db33ca8f50798126141ab2fa262d8b`; retaining the evidence itself
is necessarily a later repository commit. The strict Operator reader consumes
only its `campaign.json`, `result.json`, and `probe-evidence.json` files. It
accepts the artifact as joined-v4 and renders the settled trade, authenticated
source resolution, redemptions, and exact terminal conservation.

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
these listeners and is intentionally refused. RPC, WebSocket, faucet, gossip,
and dynamic service ports are collision-checked and can be set with the
`CLUTCH_LOCAL_REAL_PYTH_*_PORT` variables. The patched outbound QUIC/UDP client
paths remain loopback-only but retain upstream's fixed validator-client port
range; the retained probe transcript therefore claims loopback isolation, not
that every client socket is inside the configurable service ranges.

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

The VAA generator is derived from `pythnet/pythnet_sdk/src/test_utils/mod.rs`
at `pyth-network/pyth-crosschain` commit
`f50a3faf9fc5a223a22889799b2f778900f186b3`. It deliberately replaces the
upstream helper's random quorum subset with guardian indices 0 through 12, so
the signed VAA is byte-reproducible for a named publish time. Upstream is
Apache-2.0; the full attribution and license are retained in
`../svm-tests/tests/fixtures/real-pyth-local/UPSTREAM_LICENSE`, and the pinned
upstream tree contains no `NOTICE` file.
