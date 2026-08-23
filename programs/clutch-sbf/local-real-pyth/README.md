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
real ABIs with deterministic laboratory guardians, verifies a newly generated
VAA whose Wormhole-body timestamp and embedded observation use the selected
fresh boundary, and drives real receiver `PostUpdate` immediately followed by
Clutch `AppendSourceArchiveV2`. It also checks wrong-Config and wrong-feed
atomic rollback, reloads the accepted update/archive, seals the one-bucket
archive, and resolves the categorical market.

It is not a devnet price, a provider-availability test, a production source
release, current deployment evidence, or a wallet client.

Run from the repository root:

```sh
CLUTCH_LOOPBACK_TEST_VALIDATOR=/absolute/path/to/solana-test-validator \
  programs/clutch-sbf/scripts/run_local_real_pyth.sh
```

The default `source-only-v1` mode preserves the original thirteen-transaction
source campaign. The explicit joined lifecycle is:

```sh
CLUTCH_LOOPBACK_TEST_VALIDATOR=/absolute/path/to/solana-test-validator \
  CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR=/new/empty/retained-directory \
  programs/clutch-sbf/scripts/run_local_joined_pyth_lifecycle.sh
```

`joined-user-lifecycle-v1` keeps Realm, Profile, policy, immutable Terms, one
collateral mint, and one ephemeral user's ordinary collateral token account as
disclosed genesis prerequisites. The market state PDAs, Hoard token account,
and four outcome mints must be absent before submission. The campaign then
retains twenty-one signed transactions in exact order: provider/router setup;
source-spec and archive admission; `CreateMarket`, `Endow`, and `Split`; the two
atomic rollback negatives; real receiver `PostUpdate` joined to Clutch append;
seal and categorical resolve; four `RedeemInternal` calls; and `WithdrawCash`.
It checks that 64 collateral atoms return to the ephemeral local user while
position cash, all internal positions and supply, the Hoard obligation, and
Hoard token balance end at exact zero.

Trading is deliberately reported as **BLOCKED / NOT SUBSTITUTED** with reason
`missing-sealed-price-grid-and-epoch-plane`. The immutable real-Pyth-bound Terms
name a PriceGrid digest, while this campaign currently constructs no matching
sealed PriceGrid artifact, Epoch, order page, or candidate plane. `InitEpoch`
authenticates the exact grid, so the runner does not replace those missing
signed lifecycle steps with mocked or genesis-injected trading state.

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
