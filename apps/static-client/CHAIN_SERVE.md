# Live read-only chain server

`operatord chain-serve` is the chain-attached owner of Glass static assets, the
fork-aware untrusted index API, and the pure payoff compiler. It has no wallet,
signer, blockhash, transaction submission, faucet, deployment, account
creation, or persistence path.

```text
operatord compose-chain-config \
  --local-release-manifest /absolute/session/public-session.txt \
  --capability-manifest /absolute/checked-profile.json \
  --cluster-name localnet \
  --expected-genesis HASH \
  --rpc-http-url http://127.0.0.1:8899 \
  --rpc-websocket-url ws://127.0.0.1:8900 > chain.json

operatord chain-serve --config chain.json [--port 9130] [--static apps/static-client]
```

The missing session-instantiation seam is owned by
`operatord prepare-local-chain` / `launch-local-chain`; those commands create
the v6 seal from exact built-release, checked-profile, compiler, neutral-sink,
Source, and validator coordinates without entering any historical mock mode.
The separate `compose-devnet-chain-config` command accepts only the canonical
devnet deployment-manifest schema and cannot reuse a local-session manifest.
See [`REAL_INFRA_CHAIN_LAUNCH.md`](../../docs/implementation/REAL_INFRA_CHAIN_LAUNCH.md).

`chain.json` is output, not caller-authored input. The offline composer invokes
the existing capability-profile v2 checker with deployability required and
cross-checks its checker-emitted SHA-256 of sorted-key compact ASCII JSON,
profile identity, measured source commit, measured ELF digest, and exact
compiled Source identity class against the v6 local-session seal. It hashes the
actual ELF file, requires the session ownership marker and exact HTTP/WebSocket
pair, validates Program/ProgramData/slot coordinates, fixes bounded runtime
policy, and derives the workflow identity. Missing, planning, historical,
unsealed, stale-decoder, or mismatched inputs fail closed.

The central registry's exact enabled intent triples—not a second operatord or
browser allocation table—select action 26 and current General, Source/Series,
Dealer, Recovery, and Fractional surfaces. Current decoder families may still
be projected without an enabled coordinate, but that state is explicitly
non-actionable. Every output integer is a canonical decimal string. The
composer reads no wallet or browser session and has no RPC, signing, submission,
deployment, or persistence path.

The hostile decoder admits Source V3 runtime accounts and only the current
Collateral Hoard V2, ClaimLedger V3, Resolution V5, and the current General
successor versions (including Window V5, AdmissionNode V4/outer-v2,
MarketBinding V2, ClearWork V3, rent-owned OwnerSettlement V5,
SettlementReceipt V5, SettlementRoot V1, Reservation V9, and OrderPage V5).
The checked `fractional` family admits only Policy V2, Ledger V1,
Credit V2, and Tombstone V2. The reinterpreted policy/credit/tombstone V1 bytes
are withdrawn and invisible to live discovery.
It also admits only the current globally enveloped Dealer state graph (State,
funded dependencies, LP pages, leases, pots, Epoch bindings, terminal work,
tombstones, tickets, and receipts); raw historical Dealer V1 bodies are not
live accounts, and the explicitly non-production upload-stage account is not
discoverable here. Failure projections likewise decode the complete current
MarketRoot V2 semantic body and exact interval-consensus work/replay accounts,
not merely their outer tag. Withdrawn versions do not silently enter the live
projection.

Product/Series discovery includes the exact `0xaa/1` MarketLifecycleRoot and
`0xad/1` SeriesMarketLink account frames in addition to SeriesRegistry and
SeriesFunding. Dealer discovery includes the exact `0xae/1`
CoveredDealerSelection body. The browser admits only the corresponding current
Rust-emitted family/kind catalog; unknown, historical, cross-family, or
placeholder labels fail closed.

The checked release also publishes the exact `source_identity` compiled into
that ELF. The ordinary `production-inert` build reports zero registered Source
releases. Glass labels that state explicitly and refuses Source actions 1–12;
it never substitutes the mock or real-Pyth laboratory identity. Those two lab
profiles are distinct release identities and each reports exactly one compiled
registration. This count is derived from the checked identity class rather
than accepted from an operator or browser. The mock laboratory may be viewed as
an explicitly fabricated untrusted projection, but Glass refuses to construct
Source actions from it. The distinct real-Pyth lab remains non-production and
can reach only the unsigned construction boundary.

Current General fee discovery distinguishes the rent-owned carry V3,
finalization V4, payer allocation V2, and certified recipient allocation V2
from their historical frames. The immutable V4/V2 snapshots enter through
their semantic owners' hostile decoders. A live carry V3 remains explicitly
`requires-context` until its selected-fee record and canonical carry PDA are
authenticated; its raw bytes never become fee authority.

## What is checked before serving

The process makes bounded read-only calls to the explicitly configured,
untrusted RPC and refuses to bind Glass unless that one endpoint reports:

1. `getGenesisHash` equals `cluster.genesisHash`;
2. each Program and ProgramData account exists at finalized commitment;
3. the Program is executable, both accounts are owned by the pinned
   Upgradeable Loader, and the Program body links the exact ProgramData address;
4. ProgramData decodes canonically and names the configured deployment slot;
5. SHA-256 of the exact ELF suffix equals `elfSha256`.

It checks the same coordinates again after each complete scan and only then
exposes `SharedIndexApi`, so an observation that changes across the scan keeps
the projection withdrawn. These checks are consistency observations, not
cryptographic chain authentication or an RPC quorum. The decoder families and
enabled intent coordinates are derived from the checked manifest/registry join,
but their daemon projection is not a current-account runtime capability verdict.
Every account response is still an untrusted projection:
onchain execution must reload complete authoritative accounts.

## Processed finality and rollback boundary

Finalized state comes from repeated `getProgramAccounts` scans. One ordered
WebSocket reader separately owns exactly one `programSubscribe` per configured
release plus `blockSubscribe`, `slotsUpdatesSubscribe`, and `rootSubscribe`.
Before a processed generation becomes readable it:

1. opens the exact configured WebSocket URL with no redirect or proxy fallback;
2. sends `getGenesisHash` on that same connection and requires the exact selected
   genesis before sending any subscription request;
3. binds every server-assigned subscription ID to its planned request;
4. buffers notifications under the configured count and aggregate-byte bounds;
5. runs a serialized HTTP release-check → finalized scan → release-check cycle;
6. replays the buffered notifications in wire order; and
7. marks the generation live only after the complete replay succeeds.

Program rows remain buffered until their slot has a unique blockhash and is
frozen. A dead slot withdraws buffered rows and already-indexed descendants.
Each new root must be non-regressing, uniquely block-identified, free of known
dead ancestry, and descend from the previous observed root. Root arrival drains
same-slot pending rows. A well-formed exact zero-lamport/empty/non-executable
closure or a well-formed non-executable owner change creates a fork-bound,
release-specific removal observation. It masks only that release's processed
row, increments the rollback epoch, preserves the finalized baseline, and is
itself reverted if its branch dies. An unknown-address removal is recorded but
has nothing to mask. Executable owner changes, malformed changes, capacity
exhaustion, ambiguous topology, release mismatch, idle timeout, or transport
loss still withdraw the entire generation—including processed versions,
removals, fork nodes, pending rows/root, and connection-scoped subscription
IDs—and reconnect with an explicitly configured exponentially increasing,
capped backoff.

Processed output is always labeled non-final and rollbackable. It has
`authorityEligibility: false`; the processed keeper endpoint returns no actions,
and Glass refuses workflow construction until the user selects and reacquires a
finalized projection. The common read gate is withdrawn during each release-
bracketed finalized scan; otherwise the last complete finalized scan remains
available while processed transport is in backoff, registration, or replay.

`/v1/acquisition` publishes a domain-separated SHA-256 binding for each complete
HTTP and WebSocket URL plus a display coordinate containing only
scheme/authority and redacted path/query markers. It never returns raw URL
userinfo, path tokens, or query credentials. Userinfo is rejected at config
admission. Glass accepts only the daemon's redacted/hash endpoint bindings and
exact composed manifest/profile/source/Program/ProgramData/slot/ELF projection;
it has no caller-shaped RPC or release fields to compare or override. Its
displayed/copied configuration is redacted too. For
processed reads it brackets its bounded GET sequence with acquisition-state
reads and rejects a changed connection generation or rollback epoch. The
echoed verification disposition means only that the read gate follows the last
complete untrusted HTTP release bracket; it is explicitly not authority
eligibility or cryptographic chain authentication.

Matching `getGenesisHash` on the exact WebSocket connection prevents a silently
different cluster from entering that processed generation. It does not prove
that the HTTP and WebSocket services share an operator, authenticate the RPC,
or make its observations authoritative. All WebSocket observations remain
untrusted and onchain execution must independently reload authority.

The RPC plan accepts HTTPS/WSS endpoints or canonical loopback
`http://127.0.0.1:PORT` and `ws://127.0.0.1:PORT` pairs. Public reads remain
subject to configured response, account, timeout, index, notification-buffer,
polling, and reconnect bounds. The HTTP worker invokes `curl` only with read-only
JSON-RPC bodies, no redirects, an explicit protocol allowlist, proxy variables
disabled, and response limits. The WebSocket owner refuses binary/batch JSON,
caps raw messages and fragmented aggregates before JSON admission, and applies
connect, read-idle, and write deadlines. Hostname resolution occurs through the
host OS resolver before bounded TCP address attempts; `std::net` does not expose
a per-lookup DNS deadline, so resolver configuration remains a named tooling
boundary. An IP-literal loopback endpoint avoids that boundary for local use.
RPC operators must also enable `blockSubscribe` and `slotsUpdatesSubscribe`;
an absent or refused required subscription withdraws the generation and enters
the configured backoff rather than silently degrading topology.
