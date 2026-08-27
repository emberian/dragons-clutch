# Static client and zero-operator deployment

## 1. Constraint

The protocol must not require a Dragon-operated web server, database, indexer,
matcher, oracle, crank, relayer, or API. The canonical client is a reproducible
static application that can be:

- addressed immutably by IPFS CID;
- mirrored for convenience on GitHub Pages;
- downloaded and run locally;
- forked and hosted by anyone.

The application is a replaceable lens. It is never part of consensus.

## 2. Trust flow

```text
static files
  -> user-selected RPC and wallet
  -> authenticated onchain accounts
  -> locally constructed transaction
  -> onchain program independently revalidates every fact
```

The client may compile a Template, compose a payoff vector, solve a small auction,
or display a candidate simplex distribution for explanation. The program
recomputes or verifies every authoritative transition.
A malicious client may inconvenience or deceive a user about intent, but cannot
withdraw Hoard principal or bypass the program's invariant. Transaction previews
must display exact program, accounts, instruction, quantities, fees, and expected
postcondition before wallet approval.

## 3. Canonical release manifest

Every static release binds:

- application version and IPFS CID;
- bundle SHA-256 and complete asset inventory;
- source repository commit and reproducible build recipe;
- supported program IDs and verified SBF ELF hashes;
- Eggcrate, Rocq model, account-layout, and instruction-schema digests;
- supported Realm, fee-policy, source-adapter, and Market versions;
- content-security policy and dependency/SBOM digest;
- known limitations and upgrade-authority status.

Markets bind protocol semantics, not a required frontend version. A client refuses
unknown semantics rather than guessing.

GitHub Pages is a convenient mutable mirror and may advertise the newest release.
It is not the canonical integrity root. The immutable CID and manifest hash are.
Mutable DNSLink/IPNS pointers are optional discovery conveniences with explicit
authority.

## 4. RPC and discovery

No privileged RPC key is embedded. Users may choose:

- their wallet's configured RPC;
- a public endpoint;
- their own endpoint;
- an optional community endpoint.

All RPC results are untrusted until account owners, addresses, versions, slots,
and program-defined digests validate locally. The UI shows RPC commitment and
staleness.

Market discovery may use `getProgramAccounts`, exact shared links/addresses, and
optional community indexes. An index is a cache of candidates, never truth. The
client fetches and validates each referenced Market account. Avoid a global
writable onchain registry that would serialize market creation merely to improve
discovery.

Anyone may publish immutable IPFS index snapshots. Each record carries its
onchain cutoff and content digest; omission is expected and never interpreted as
nonexistence.

## 5. Permissionless workbench

The client exposes paid public work when available:

- post a qualifying source update;
- advance or repair a feed bucket;
- fold an archive page into a WindowResult;
- propose or continue batch verification;
- allocate a final batch page;
- finalize a Hatch;
- settle a Position;
- retire an empty Market.

The UI estimates network expense and frozen reward without promising inclusion.
It does not hold keeper keys, schedule background transactions, or submit without
the user's explicit wallet approval. Headless third-party keepers use the same
public instructions and earn under the same rules.

For the native venue, the client distinguishes three things that must never be
collapsed: the final verified simplex price vector, an unfinalized submitted
candidate, and external per-Egg spot quotes. A portfolio composer shows its exact
coefficient vector, maximum payout, implied cost at each available venue, fees,
and rounding before signature. Human labels such as “crash hedge” are presentation
only; the canonical vector and Template digest are authoritative.

## 6. Static security posture

- No remote JavaScript, analytics, pixels, cookies, or dynamic tag manager.
- Strict CSP; only the selected RPC/gateway/wallet transports are connectable.
- Dependencies bundled, pinned, inventoried, and reproducibly built.
- No private key, seed phrase, Pump key, provider credential, or session secret.
- No server-side rendering assumption.
- Explicit cluster/program identity on every signing surface.
- Human-readable and raw transaction inspection.
- Fail closed on manifest, schema, program, or Realm mismatch.
- Service worker caches only exact hashed assets and never silently upgrades an
  active session.
- A downloadable local build and instructions for independent hash verification.

IPFS availability still requires somebody to pin content, and an RPC/gateway is
still infrastructure. The claim is not “the internet vanished”; it is that no
unique Dragon service is required and every dependency is replaceable.

## 7. Accessibility

The static constraint must not reproduce inaccessible trading interfaces. The
client should be keyboard-complete, screen-reader legible, responsive without tiny
tap targets, tolerant of motor impairment, and explicit about price/payout/fee
units. Every visual chart has a structured text/table equivalent. A user can
inspect, simulate, and prepare a transaction without time pressure; batch
deadlines and repair windows are presented in absolute and relative form.
