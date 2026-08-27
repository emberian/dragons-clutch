# Mainnet state on devnet: trust roots, transports, and the v1 relayer

Status: design and verified research for the cross-cluster capability named in
`WAVE.md` ("the demo is the completed dClutch"). It specifies one new provider
family and decides which product classes need it. It is not an ADR, not a
release manifest, and not deployment evidence. No `docs/OMISSION_INDEX.md` row
is closed by it.

Every claim below carries one of: **verified-on-chain** (a bounded public RPC
read logged in §8), **verified-from-source** (read in a local crate checkout or
this repository), **verified-from-docs**, **secondhand**, or
**could-not-verify**. Every fixed number carries one of: *mathematical*,
*chain-derived*, *measured-profile*, or *provisional* per `AGENTS.md`.

---

## 0. What this document decides

1. Majors price markets need no cross-cluster transport at all, and the reason
   is architectural rather than operational (§1.3, §2).
2. A new Source provider family, `RelayedMainnetStateV1`, whose provider is a
   disclosed proof-of-authority relayer that signs **observations of mainnet
   account bytes** and never interpretations (§4).
3. The trust root is swappable without moving semantics, and the statement of
   that property is checkable rather than rhetorical: `decoding_rules_id` is
   byte-identical across trust roots (§4.10).
4. The product-to-transport map and the smallest first slice (§5, §6).

It does **not** decide: whether to deploy anything, the κ capacity constant
from `docs/research/CHAIN_STATE_SOURCES_2026_08.md` §6.5, or the venue field
offsets (§7).

---

## 1. The cross-cluster problem, stated exactly

### 1.1 What a cluster boundary is

A program executing on devnet has exactly three sources of fact: the accounts
the transaction names, the sysvars, and its own instruction data. All three are
devnet-local. There is no syscall, no CPI, no sysvar, and no loader path that
exposes another cluster's state. This is structural rather than a missing
feature: an account is identified by a pubkey within one ledger, and no SVM
primitive takes a ledger as an argument.

Therefore every "devnet program resolves mainnet state" scheme has the same
shape, and only one degree of freedom:

```text
trust root  +  message  +  on-devnet verification predicate
```

The message is always instruction data or account bytes on devnet. The
verification predicate is always a signature check plus a byte-exact structural
check. **The only thing that differs between candidate transports is whose keys
sign, and what those keys are claiming.** Comparing transports is therefore
comparing trust roots, not comparing technology.

### 1.2 The available trust roots

| | Trust root | What the signature claims | New trust for dClutch |
| --- | --- | --- | --- |
| T-1 | Pythnet publisher aggregate → a 3-of-5 Pyth-controlled guardian multisig → Pyth receiver on devnet | "the aggregate price of asset X at time t" | see §2 |
| T-2 | Wormhole guardian set, via Queries — but on devnet this is **one test key**, §3.2 | "mainnet account A had bytes B at slot s" | see §3: **not available** |
| T-3 | A disclosed dClutch relayer key set | "mainnet account A had bytes B at slot s" | one key set, disclosed per market (§4.9) |

T-2 and T-3 make the *same kind* of claim and differ only in who signs it.
That is why §4's wire is designed so that changing the signer changes the
provider release and nothing downstream of it — even though §3 finds T-2 has no
occupant today.

### 1.3 The reframe: most "mainnet" products are not mainnet-account products

This is the single most useful finding in this lane, and it removes work rather
than adding it.

A market's claim basis is a *proposition*, not a location. "SOL/USD is above
$200 at time T" is a proposition about a price. It is not a proposition about
the contents of any mainnet account. Solana mainnet happens to be a place where
Pyth *also* posts that price, but the market does not care, and binding the
Product to a mainnet account would be strictly worse: it would make a routine
Pyth receiver upgrade on mainnet into a market failure event, for no gain.

The propositions that genuinely require mainnet account bytes are the ones
whose *subject* is a mainnet account:

- "mint M's Meteora DBC pool reached `MigrationProgress::CreatedPool`" — the
  pool is a specific mainnet account and there is no cluster-independent
  statement of the same fact;
- "the pump.fun bonding curve for mint M has `complete == true`";
- "the reserves of mainnet pool P imply a price above X".

So the split is:

```text
claim about a PRICE of a major asset      -> no cross-cluster transport (T-1)
claim about a MAINNET ACCOUNT's contents  -> cross-cluster transport (T-3 only)
```

Everything in §4 exists only for the second row.

---

## 2. T-1 — Pyth on Solana devnet

### 2.1 Verdict

**Confirmed: Solana devnet carries live, mainnet-derived Pyth prices, under the
same program IDs, the same Pythnet emitter, and a byte-identical guardian set
as mainnet-beta. A devnet dClutch adapter can resolve a SOL/USD market against
a real market price with no relayer.** (verified-on-chain, §8 reads 7–30.)

Three qualifications change how the path must be built, and the first is
urgent.

### 2.2 The Pyth Core cutover landed 2026-08-26 16:00:49 UTC

This lane read chain state roughly ten hours after a hard generation change,
and the repository's pinned Pyth evidence is on the wrong side of it.

| | legacy generation | upgraded generation |
| --- | --- | --- |
| Solana receiver | `rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ` | `rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp` |
| Wormhole receiver | `HDwcJBJXjL9FpJ7UBsYBtaDjsBUhuLCUYoz3zr8SWWaQ` | `HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL` |
| push-oracle / price-feed program | legacy `pythWS…` | `pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou` |
| SOL/USD shard-0 account | `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` | `7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE` |

Both generations' program IDs are identical on devnet and mainnet-beta
(verified-on-chain). Per-feed account addresses changed because they are PDAs
of the push-oracle program under seeds `[shard_id: u16 LE, feed_id: [u8; 32]]`,
and that program ID changed; the derivation was reproduced independently in
this lane and matches the documented addresses for SOL, BTC, ETH and USDC
(verified-from-source).

Three consequences for this repository, all actionable and none of them this
lane's to execute:

1. `docs/evidence/PYTH_SYNTHETIC_RELEASE_V1.md` pins a receiver ABI identity
   captured from upstream commit `f50a3faf…`. That is the **legacy**
   generation. It remains valid as local fixture evidence — it was never a
   cluster release claim — but no production release row may be derived from it
   for either cluster now.
2. The legacy binaries **differ between clusters**: devnet's legacy receiver was
   redeployed 2026-08-25T14:38:48Z while mainnet's still carried a
   2025-03-16 binary, and the two clusters' legacy Wormhole-receiver
   `ProgramData` allocations differ (1,356,397 vs 1,355,373 bytes)
   (verified-on-chain). The upgraded generation, by contrast, has **identical**
   `ProgramData` sizes on both clusters, deployed 2026-05-05. Any pinned Pyth
   release must therefore be pinned **per cluster**, never assumed shared.
3. Mainnet's legacy Wormhole receiver was upgraded at 2026-08-26T16:00:49Z.
   Under policy P-B, a live market pinned to its pre-upgrade `elf_digest` would
   have walked to its failure outcome yesterday. That is not a criticism of
   P-B; it is the measured base rate that the Product's failure-outcome
   disclosure has to survive.

### 2.3 Devnet prices are the real Pythnet aggregate

The proof is the receiver `Config` PDA, read on both clusters and both
generations (verified-on-chain). All four are identical in the fields that
decide it:

```text
num_data_sources             = 1
  chain                      = 26  (Pythnet)
  emitter                    = 6R92oFT6UiP2xWZBjTbwAkHzFCLy5BhWnNh6m83ndhZR
single_update_fee_in_lamports = 0
minimum_signatures           = 3
```

The devnet receiver admits **exactly one** data source, and it is the same
Pythnet aggregation emitter mainnet admits. A synthetic devnet feed would
require a second data source or a different emitter; there is neither. The
observed accounts corroborate it: both clusters' SOL/USD accounts are 134-byte
`PriceUpdateV2` records (discriminator `22f123639d7ef4cd`, `expo = -8`) at
`verification_level = Full`, reading $101.663986 (devnet, 01:39:25Z) against
$101.325000 (mainnet, 01:44:24Z) — the same real series.

### 2.4 The trust root moved, and it is smaller than the repository assumes

`docs/evidence/PYTH_SYNTHETIC_RELEASE_V1.md` contains this sentence:

> The lab release therefore must not compare the Config value five with the
> 19-guardian strict-majority threshold ten.

That framing is now stale. The guardian set backing Pyth on both clusters is
**index 0, five keys, `expiration_time = 0`, byte-identical across devnet and
mainnet-beta**, and guardian sets 1 through 5 have already been closed
(verified-on-chain). The migration reached mainnet at 2026-08-26T16:02:14Z and
devnet 9 minutes 34 seconds later. Bridge config on both clusters and both
program generations: `guardian_set_index = 0`, `fee = 0`.

So the Pyth trust root on Solana is now a **3-of-5 Pyth-controlled multisig**,
not a strict majority of nineteen Wormhole guardians. Two things follow and
both belong in the Product, not in a footnote:

- The "zero new trust" claim for majors is exact but must be stated as *zero
  trust beyond what a Pyth-sourced market already accepts*, and what that is
  has just become materially smaller. A devnet market and a mainnet market now
  rest on the identical five keys, which is what makes the devnet path sound;
  it is also the entire security floor.
- The comparison the evidence file forbids is no longer the relevant one.
  `minimum_signatures = 3` and the guardian-set cardinality 5 are now the same
  quorum question, and the file's quorum-distinction paragraph should be
  revisited by its owner.

### 2.5 Devnet is cranked ~20–100× slower than mainnet, by a different pusher

Measured over the last twelve updates to each account (verified-on-chain):

| | devnet | mainnet |
| --- | --- | --- |
| inter-update gaps | 310, 311, 311, 313, 314, 314, 315, 315, 316, 318 s | 0, 0, 2, 3, 6, 6, 9, 9, 9, 9, 10 s |
| sole fee payer | `4p16wya1Vw2u9w22oah4yXQgySb6eWKRRLMsEXCreish` | `9F6ApEtzkHVdZXzsury6BYmyEh4pahDBxuhNLaGC6saC` |

Pyth's documentation states a 1-minute-heartbeat / 0.5%-deviation policy for
sponsored feeds and states that sponsored feeds cover "Solana mainnet and
devnet" (verified-from-docs). The measured devnet cadence of **~315 s** does not
match that policy and the divergence is **undocumented**
(measured-profile, n = 12, single observation window).

Two design consequences:

- Any devnet majors Product must set `WindowSpecV1.max_age_seconds` from this
  measurement, not from the documentation. A 30-second staleness bound — the
  value in Pyth's own integration example — would reject essentially every
  devnet read. Budget **≥ 400 s** on devnet and make it a Product parameter, so
  that the same Product compiled against mainnet can tighten it to seconds.
  *measured-profile, provisional*: one observation window is not a bound; the
  first slice must re-measure over a longer period and report the maximum
  observed gap.
- The devnet pusher is a single third-party key we do not control. The
  guardian signature means it cannot forge a price; it fully controls **which
  signed update gets posted and when**, so freshness is a consumer-side
  obligation. That is exactly where `WindowSpecV1` already puts it.

### 2.6 What "no relayer" does and does not buy

It buys the removal of a signing party. It does not buy a liveness guarantee.
The devnet crank is a third-party service that Pyth's own documentation
describes as best-effort — *"these push feeds may occasionally experience
delays…"*, *"Applications are advised to run their own price-pusher"*, *"Push
feeds are subject to change with prior notice"* (verified-from-docs) — and the
self-service escape hatch has just been priced: Hermes, the endpoint an
application would use to fetch update payloads and run its own pusher, now
returns **HTTP 401 `unauthorized`** without an API key on both
`hermes.pyth.network` and `pyth.dourolabs.app/hermes` (verified by probe, §8).
No unauthenticated devnet Hermes endpoint is documented (could-not-verify that
a free tier exists).

So a majors market on devnet needs no relayer and no new trust, and still needs
the funded permissionless failure path of §4.8 for exactly the same reason a
relayed market does: the party that keeps the data flowing can stop. The
architecture already treats provider transport as separate from Product truth
(`ARCHITECTURE.md`), and this is the case that shows why.

---

## 3. T-2 — Wormhole Queries

### 3.1 Verdict

**Wormhole Queries does support Solana account reads, and a Solana program can
verify a guardian-signed response. It cannot do what dClutch needs, for four
independent reasons, any one of which is disqualifying. It is not a candidate
for v1 and it is not a near-term upgrade path. "Permissionless" is not real
here.**

The four reasons are ordered by how hard they are to route around; the first
cannot be routed around at all.

### 3.2 Reason 1 — the devnet trust root is a single test key, and the guardian sets do not match

This is the decisive fact and it is measured, not inferred (verified-on-chain,
§8 reads 31–36).

| | Solana mainnet-beta | Solana devnet |
| --- | --- | --- |
| core bridge program | `worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth` | `3u8hJUVTA4jH1wYAyUur7FFZVQ8H635K3tSHHF4ssjQ5` |
| active `guardian_set_index` | **7** | **0** |
| `GuardianSet` account size | 396 bytes | **36 bytes** |
| guardian keys | **19** | **1** |
| quorum `(n*2)/3 + 1` | **13** | **1** |

The single devnet key is `0x13947Bd48b18E53fdAeEe77F3473391aC727C638`, the
Wormhole **testnet** guardian, which appears as `INIT_SIGNERS` in the
repository's testnet environment files (verified-from-source).

Note also that `worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth` is **mainnet-only**;
on devnet that pubkey is a plain system-owned account with `executable: false`
and zero data (verified-on-chain).

A mainnet query response is signed by ≥13 of mainnet guardian set 7. Any
on-devnet verifier derives its `GuardianSet` PDA from the devnet core bridge,
finds one key, and refuses every signature at guardian index ≥ 1 immediately.
**There is no governance path to install the mainnet guardian set on devnet.**
So the exact thing this lane was sent to evaluate — verifying a *mainnet*
account read on *devnet* — is not merely unbuilt; it is unavailable.

And the fallback reading is worse, not better. A devnet demo built on the
devnet core bridge has a trust root of **one foundation-held test key with
quorum 1**. That is strictly weaker than the v1 PoA relayer of §4, which is at
least disclosed in the Market's own identity, immutable for the market's life,
and publicly falsifiable against mainnet. A green devnet Queries run would look
identical to a real one and would prove nothing about the 13-of-19 assumption.
Saying that out loud is the point of measuring it.

### 3.3 Reason 2 — Solana account queries are experimental by construction

From the protocol's own whitepaper, still present at HEAD
(`whitepapers/0013_ccq.md`, verified-from-source):

> An experimental implementation of queries for Solana is being added as of
> January, 2024. This implementation is considered experimental because
> **Solana does not natively support reading account data for a specific slot
> number, meaning each guardiand watcher will return data for its version of
> the most recent slot, possibly making it difficult to reach consensus.**

This is load-bearing rather than a stale caveat. Quorum requires ≥13 guardians
to sign the **byte-identical digest**, and the signed bytes include
`slot_number`, `block_time`, and `block_hash` (verified-from-source). Nineteen
nodes each calling `getMultipleAccounts` at their own current finalized slot
will generally produce nineteen different digests. `min_context_slot` sets a
lower bound; it does not force equality, and no slot-pinning mechanism exists.
The empirical mainnet quorum success rate for `sol_account` is
**could-not-verify** without an API key.

For dClutch this is precisely the wrong failure shape. §4.11 makes
same-finalized-slot atomicity a *daemon obligation* discharged by a single
`getMultipleAccounts`; Queries makes it a *consensus problem* among nineteen
independent readers, with no mechanism to solve it.

### 3.4 Reason 3 — it is triple-gated, with no wildcard for Solana

Three independent access gates, all verified-from-source in the guardian node:

1. **Guardian-level allowlist.** `--ccqAllowedRequesters` is a mandatory,
   non-empty list of signer addresses; the node refuses to start with CCQ
   enabled and an empty list. Guardians do not reply at all to unauthorised
   requests.
2. **Proxy API key**, `X-API-Key`. Access is a closed beta behind an
   application form; **cost is could-not-verify**, nothing is published.
3. **Per-call allowlist inside the key.** For Solana the permission entries are
   `solAccount:<chain>:<base58 account>` and `solPDA:<chain>:<base58 program>`,
   and the address must parse as valid base58 or 0x-hex-32. Unlike `ethCall`'s
   `contractAddress`, **there is no wildcard for Solana**, and `allowAnything`
   is hard-blocked on mainnet in code.

So every individual mainnet pool a dClutch market wanted to read would have to
be registered by the Wormhole Foundation in a proxy config, one pubkey at a
time, before that market could exist. A permissionless market factory cannot
be built on a per-account human approval queue. This is the sense in which the
word "permissionless" fails: not partially, but at the exact point where
dClutch needs it.

### 3.5 Reason 4 — the query code has been on maintenance-only since 2024

Most recent *substantive* changes by path (verified-from-source, GitHub commit
history at `f42dea01`, 2026-08-25):

| path | last substantive change |
| --- | --- |
| `node/pkg/watchers/solana/ccq.go` | **2024-08-12** |
| `sdk/js-query/` | **2024-12-04** (still version 0.0.14) |
| `whitepapers/0013_ccq.md` | **2024-10-08** |
| `node/cmd/ccq/` | 2026-05-29 |

The Rust query SDK is a git dependency at rev `0f34cb47` (last commit
2024-06-21), version 0.0.1, **not published on crates.io**, and its
`ChainSpecificQuery` enum is missing the `sol_pda` variant entirely. The one
published Solana verification PoC last moved 2024-07-24. Docs still say closed
beta in 2026. Not deprecated; frozen.

### 3.6 What is worth keeping from this investigation

Two things, and they are worth more than the negative verdict.

**The Verify VAA Shim is a reusable digest verifier.** Program
`EFaNWErqAtVWufdNb7yofSHHfWFos843DFpu4JBw24at`, deployed and executable on
**both** mainnet-beta and devnet at the same address (verified-on-chain). Its
`VerifyHash` instruction takes an **arbitrary 32-byte digest** and checks
signatures against the core bridge's guardian set, and its own documentation
states it is "compatible with both v1 VAAs and Queries." If a future dClutch
transport ever needs secp256k1 quorum verification against a guardian-style
key set, this is the primitive, and it does not require the query protocol.

**Measured verification cost, for calibration** (from the shim's published cost
table, 13 signatures against a 19-guardian set; verified-from-docs):

| | CU | lamports |
| --- | --- | --- |
| `post_signatures` | 3,037 | 7,206,592 rent, refunded |
| `verify_hash` + `close_signatures` | 334,846 | −7,191,616 refund |
| total | **337,883** | **15,040 net** |

Consistent with `secp256k1_recover_cost = 25,000` CU × 13 = 325,000
(verified-from-docs, Agave `execution_budget.rs`). Two transactions; the CU
limit must be raised above the 200,000 default.

Set that against §4.4's Ed25519 path: **2,400 CU per signature charged by the
block cost model, and zero CU charged against the transaction's own compute
budget.** Per signature that is 25,000 against 2,400 — a factor of 10.4 — but
the ratio is not the important part. The important part is *which budget it
comes out of*: thirteen `secp256k1_recover` calls consume 325,000 CU of the
1,400,000 program ceiling, and thirteen Ed25519 precompile verifications
consume none of it. Given that `WAVE.md` records the common Hot path already
fighting that ceiling, this is a concrete reason to keep the relayer family on
Ed25519 rather than adopt a guardian-shaped signature scheme for its own sake.

### 3.7 Consequence for §4

None of §4 changes. The design was already built so that the trust root is one
content ID (§4.10), and the outcome of this investigation is that the fourth
row of that table has no occupant today. The row stays, because the *shape* is
what makes the swap cheap, and because the shim in §3.6 shows the verification
primitive exists independently of the query protocol.

---

## 4. T-3 — `RelayedMainnetStateV1`, the v1 proof-of-authority family

### 4.1 The one rule the family exists to enforce

The relayer signs **observations**. It never signs interpretations.

Concretely, the signed message contains only:

- pubkeys the relayer did not choose (the ordered set is pinned by the
  immutable release, §4.5);
- each account's `owner`, `lamports`, `executable`, and exact `data_len` as
  read;
- a release-pinned inline prefix of each account's `data`;
- SHA-256 over the remainder of that account's `data`;
- the finalized slot the read was taken at;
- the genesis hash of the cluster read.

No field is selected, scaled, compared, thresholded, or named by the relayer.
Every layout fact — discriminators, admitted length sets, field offsets,
sentinels, fixed-point scale, rounding boundary — lives in the
`decoding_rules_id` record and is applied by the on-devnet adapter, exactly as
`CHAIN_STATE_SOURCES_2026_08.md` §6.3 requires.

This is enforceable rather than aspirational, and the release check is a
one-liner: **the relayer daemon's dependency closure must contain no venue IDL,
SDK, or layout crate.** A relayer that cannot parse a `PoolState` cannot
interpret one.

### 4.2 What must cross the boundary

For the DBC graduation product the ordered account set is four accounts, and
each is there for a reason that cannot be satisfied any other way:

| # | Account | Why it must be attested |
| --- | --- | --- |
| 1 | venue `Program` (Loader V3, 36 bytes) | yields the observed `programdata_link` — read out of the account, never supplied |
| 2 | venue `ProgramData`, first 45 bytes inline | yields `deployment_slot` and `upgrade_authority`; its **tail digest is exactly the registry contract's `elf_digest`** (§4.4) |
| 3 | the venue state account (`PoolState`, 416 bytes) | the subject of the claim |
| 4 | mainnet `Clock` sysvar (40 bytes) | mainnet wall time as a *decoded observation*, never a relayer claim (§4.7) |

Account 4 deserves emphasis. The window in `WindowSpecV1` is expressed in Unix
seconds and the proposition is about *mainnet* time. Devnet's `Clock` is a
different cluster's clock. Attesting mainnet's `Clock` sysvar account makes
mainnet time an ordinary decoded field of an ordinary attested account, subject
to the same decoding-rules discipline as everything else, rather than a
timestamp the relayer asserts.

### 4.3 The attestation wire

Three signed message kinds and two immutable records. Every one is
fixed-prefix, little-endian, reserved-bytes-must-be-zero, and hostile-decoded
by exact width.

**`RelayedAccountObservationV1`** — the per-account observation body, 112-byte
head plus an inline tail:

```text
off  len  field
  0   32  key
 32   32  owner
 64    8  lamports              u64
 72    4  data_len              u32   exact full on-chain data length
 76    2  inline_len            u16   0 <= inline_len <= min(data_len, 448)
 78    1  executable            0 | 1
 79    1  reserved              zero
 80   32  tail_digest           SHA-256 over data[inline_len .. data_len]
112   ..  inline                exactly inline_len bytes = data[0 .. inline_len]
```

Three properties follow from this shape and they are the reason it is shaped
this way:

- A fully inline account sets `inline_len == data_len` and `tail_digest` to the
  SHA-256 of the empty string; the adapter recomputes it and requires equality.
  There is no special case and no variant tag.
- The relayer commits to the **complete** account either way. Omitted bytes are
  digested, not dropped, so "inline a prefix" is a carriage decision, never a
  content decision.
- For a Loader-V3 `ProgramData` account with `inline_len = 45`, `tail_digest`
  is by construction SHA-256 over `data[45..]`, which is *precisely*
  `DeploymentObservationV1.elf_digest` as `crates/dclutch-registry-svm`
  already defines it (`LOADER_V3_PROGRAMDATA_METADATA_BYTES = 45`,
  `ProgramDataV3View::elf()` borrows "the complete ProgramData byte tail
  beginning at fixed offset 45"). (verified-from-source.) A 2.3 MB mainnet
  program therefore costs 157 wire bytes and needs no new authentication
  primitive.

**`RelayedMainnetAccountAttestationV1`** (magic `DCLTRMA1`) — one signer, one
account:

```text
off  len  field
  0    8  magic                 b"DCLTRMA1"
  8    2  abi_version           u16 = 1
 10    2  reserved              zero
 12    4  message_len           u32   exact; must equal the verified message length
 16   32  observed_cluster_id   genesis hash of the cluster read
 48   32  relay_family_id       = ProviderReleaseV1.provider_family_id
 80   32  decoding_rules_id     = ProviderReleaseV1.decoding_rules_id
112   32  account_set_id        = RelayedAdapterConfigV1.account_set_id
144    8  observed_slot         u64, finalized
152    2  set_index             u16, position in the canonical ordered set
154    2  set_count             u16
156   ..  RelayedAccountObservationV1 body (112 + inline_len)
```

Fixed cost 268 bytes plus `inline_len`.

**`RelayedObservationSetV1`** (magic `DCLTRMS1`) — one signer, the whole
ordered set in one message. This is the one-transaction profile's wire
(§4.4) and it exists only where the geometry admits it:

```text
off  len  field
  0    8  magic                 b"DCLTRMS1"
  8    2  abi_version           u16 = 1
 10    2  reserved              zero
 12    4  message_len           u32   exact
 16   32  observed_cluster_id
 48   32  relay_family_id
 80   32  decoding_rules_id
112   32  account_set_id
144    8  observed_slot         u64, finalized
152    2  set_count             u16
154    2  reserved              zero
156   ..  set_count ordered RelayedAccountObservationV1 bodies
```

Fixed cost 156 bytes plus `sum_i (112 + inline_len_i)`. Because m signers over
one message would produce m identical message slices, which §4.4's parser
refuses by construction, **this profile is `seal_threshold = 1` only.**

**`RelayedObservationSetSealV1`** (magic `DCLTRSS1`) — one signer, seals a
completed set. Exactly 156 bytes:

```text
off  len  field
  0    8  magic                 b"DCLTRSS1"
  8    2  abi_version           u16 = 1
 10    2  reserved              zero
 12    4  message_len           u32 = 156
 16   32  observed_cluster_id
 48   32  relay_family_id
 80   32  account_set_id
112    8  observed_slot         u64
120    2  set_count             u16
122    2  reserved              zero
124   32  set_digest
```

`account_set_id` is the founding-time pin of *which* accounts may be attested:

```text
account_set_id = SHA-256(
    b"dclutch/relayed-account-set/v1" || 00 ||
    observed_cluster_id (32)          || 00 ||
    relay_family_id (32)              || 00 ||
    set_count (u16 LE)                || 00 ||
    for i in 0..set_count:
        key_i (32) || expected_owner_i (32) || inline_len_i (u16 LE)
)
```

The relayer chooses none of these. It echoes them and the adapter compares.

`set_digest` folds the accepted bodies in order, so no large on-chain hash is
ever needed:

```text
running_0     = SHA-256(b"dclutch/relayed-set/v1" || 00 ||
                        account_set_id || observed_slot (u64 LE))
running_{i+1} = SHA-256(running_i || body_i)
set_digest    = running_{set_count}
```

**`RelayedAdapterConfigV1`** — 64 bytes, the exact analogue of the existing
64-byte `PythAdapterConfigV1`:

```text
off  len  field
  0   32  account_set_id
 32    4  observable_selector      u32, index into the decoding-rules observable table
 36    4  raw_exponent             i32, declared scale of the produced atom
 40    8  max_observation_age_seconds  u64
 48    8  max_cluster_skew_seconds     u64
 56    8  reserved                 zero
```

**`RelayerKeySetV1`** (magic `DCLTRKS1`) — the immutable record whose content ID
*is* `ProviderReleaseV1.provider_deployment_release_id` (§4.5):

```text
off  len  field
  0    8  magic                 b"DCLTRKS1"
  8    2  abi_version           u16 = 1
 10    1  key_count             u8, 1 ..= 5
 11    1  seal_threshold        u8, 1 ..= key_count
 12    4  reserved              zero
 16  160  keys[5]               32 bytes each; unused slots must be zero
```

Total 176 bytes, matching `ProviderReleaseV1`'s own width. Keys are strictly
ascending as byte strings, so the set is canonical and duplicate keys are
structurally impossible.

### 4.4 Transport profiles and packet geometry

The signature primitive is the Solana Ed25519 native precompile plus
Instructions-sysvar introspection. Three facts fix the geometry
(all verified-from-source in the local crate checkout):

- `agave-precompiles-4.3.0-beta.2/src/lib.rs::verify_if_precompile` verifies a
  precompile instruction against `all_instructions`, the transaction's
  **top-level** instruction list, during transaction verification. Precompiles
  are not reachable by CPI. The post-then-consume-inside-one-instruction
  transport that `docs/compost/PYTH_LOCAL_UPGRADED_2026_08_22.md` chose for
  Pyth is therefore *not available* for an Ed25519 attestation; adjacency is
  the only transport.
- `ED25519_VERIFY_STRICT_COST = COMPUTE_UNIT_TO_US_RATIO * 80 = 2,400`
  (`solana-cost-model-4.3.0-beta.2/src/block_cost_limits.rs`). This is charged
  by the **block** cost model (`CostModel::get_signature_cost`), not against
  the transaction's own compute budget. Signature verification therefore costs
  the dClutch program zero CU and costs the block 2,400 CU per signature.
  *chain-derived.*
- `SIGNATURE_OFFSETS_SERIALIZED_SIZE = 14`, `SIGNATURE_OFFSETS_START = 2`,
  pubkey 32, signature 64 (`solana-ed25519-program-3.0.0`). An m-signature
  precompile instruction is `2 + 110m` data bytes.

**O-018 compliance, stated before the design leans on adjacency.** O-018 forbids
Instructions-sysvar *adjacency as the authority*. It is not the authority here.
The authority is the pair (immutable release-pinned public key, byte-exact
message equality against the current instruction's own data). Adjacency only
selects which instruction to parse. This is exactly the discipline already
implemented for Direct in
`crates/dclutch-direct-contract/src/adapter.rs::inspect_preceding_ed25519_batch_item_v2`,
which requires the preceding program ID to equal the Ed25519 program, requires
`preceding_index + 1 == current_index`, requires every descriptor field to
match by exact equality, requires each message slice to lie inside the *current*
instruction's data, refuses an all-zero signature, and refuses overlapping
message slices. (verified-from-source.)

**One implementation obligation found here, not later.** That helper refuses
*overlapping* message slices. An m-of-n key set signing the **same** message
produces m identical slices, which the existing helper rejects by construction.
The family therefore does not batch m signatures over one message; it uses one
short seal message per signer, one signer per transaction (§4.3). At m = 1 the
distinction is invisible; specifying it now is what keeps it from becoming a
surprise at m = 3.

**Packet arithmetic.** The hard ceiling is `SOLANA_PACKET_DATA_SIZE_3_0 =
1,232` (already pinned in `dclutch-direct-contract`). *chain-derived.* For a v0
transaction carrying one fee-payer signature, four static keys (fee payer,
Ed25519 program, resolution program, ComputeBudget program), a 32-byte
blockhash, one address-lookup-table entry covering nine looked-up accounts, two
ComputeBudget instructions, one 1-signature Ed25519 instruction, and the append
instruction:

```text
signatures                        65
message header                     3
static keys (4)                  129
blockhash                         32
ALT section (1 table, 9 keys)     44
instruction count                  1
ComputeBudget x2                  22
Ed25519 (1 signature)            116
append ix framing (9 accounts)    13
                                ----
                                 425     leaving 807 bytes of instruction data
```

The append wire is a 64-byte Source-owned prefix (matching
`AcceptSharedObservation`) plus the message, so the message budget is 743 and
the inline budget is `743 - 268 = 475`.

**`MAX_RELAYED_INLINE_BYTES_V1 = 448`** — *chain-derived ceiling (1,232 packet)
with provisional frame arithmetic*; 27 bytes of headroom against frame
variation, and it must be re-derived from a measured `ProgramTest` frame before
release. It admits the DBC `PoolState` at 416 bytes fully inline, which is the
number that makes the first slice work at all.

**`MAX_RELAYED_ACCOUNTS_V1 = 8`** — *provisional*; chosen to cover the largest
account set in `CHAIN_STATE_SOURCES_2026_08.md` §2.1 (Raydium CPMM: pool + two
vaults, plus program, programdata, clock = six) with headroom. Lifting plan:
raise only against a measured venue that needs it.

Two profiles, mirroring the existing `SourceAccessProfile` split:

- **`RelayedTerminalOneTransaction`** — the whole set rides in one
  `RelayedObservationSetV1` message in one transaction. Available only when
  `156 + sum_i (112 + inline_len_i) <= 743`, and only at `seal_threshold = 1`.
  The DBC set needs `156 + 148 + 157 + 528 + 152 = 1,141` bytes and therefore
  **does not qualify**.
- **`RelayedObservationRecord`** — the family-general profile. One append
  transaction per account into a program-owned record, then one seal
  transaction per signer, then resolution consumes the sealed record. No size
  cliff. This is the profile the first slice uses.

The relationship is the same one `CHAIN_STATE_SOURCES_2026_08.md` §6.4 draws
between its Mechanisms A and B: the one-transaction profile is a per-venue
optimization admitted where the geometry allows it, never the family's general
mechanism.

**`RelayedObservationRecordV1`** (magic `DCLTROB1`) is the record profile's
persisted state: a program-owned devnet account and a **direct Market child**,
following `SharedObservationStateV1`'s discipline exactly — expected
child-count on creation with one checked `before -> before + 1` delta, one
checked `before -> before - 1` on retirement, and a pre-existing RentCredit
beneficiary. Its PDA seed tuple is

```text
b"dclutch/relayed-obs/v1", market, generation (u64 LE), account_set_id,
observed_slot (u64 LE), bump
```

Seeding by `observed_slot` is what makes equivocation structurally bounded
(§4.8): at most one record exists per set per slot.

312-byte header plus `MAX_RELAYED_ACCOUNTS_V1` fixed slots:

```text
off  len  field
  0    8  magic                     b"DCLTROB1"
  8    2  abi_version               u16 = 1
 10    2  reserved                  zero
 12   32  market
 44    8  generation                u64
 52   32  source_material_id
 84   32  account_set_id
116   32  provider_release_id
148   32  relayer_key_set_id
180   32  observed_cluster_id
212    8  observed_slot             u64
220   32  set_digest                running fold; final after the last append
252   32  rent_credit_beneficiary
284    8  created_unix_seconds      i64   devnet Clock
292    8  sealed_unix_seconds       i64   devnet Clock; zero until sealed
300    2  set_count                 u16
302    2  filled_count              u16
304    1  seal_threshold            u8    echoed from RelayerKeySetV1
305    1  seal_count                u8
306    1  sealed_by_bitmap          u8    which key-set members have sealed
307    1  phase                     u8    Collecting=1 Sealed=2 Consumed=3 Retired=4
308    4  reserved                  zero
312   ..  slots[set_count]          stride 560 = 112-byte body head + 448-byte
                                    inline region, zero-padded beyond inline_len
```

At `MAX_RELAYED_ACCOUNTS_V1 = 8` the record is `312 + 8 × 560 = 4,792` bytes.

Appends fill strictly increasing `set_index` — the same rule
`SharedObservationStateV1` uses for schedule indices — so the fold order is
canonical and a repeat is a replay refusal rather than an overwrite. Only the
transition into `Sealed` may set `sealed_unix_seconds`, and only a `Sealed`
record may be consumed by a resolution. Retirement can close a `Collecting`,
`Sealed`, or `Consumed` record into its RentCredit beneficiary.

Ordered account frames, exact counts, no aliases, exact signer/writable
privileges:

| Route | Count | Frame |
| --- | --- | --- |
| create | 11 | worker (signer, writable) · Market (writable) · record (writable) · raw `SourceMaterialV2` · raw `SourceSpecV1` · raw `ProviderReleaseV1` · raw `RelayerKeySetV1` · raw `RelayedAdapterConfigV1` · RentCredit · Rent sysvar · System Program |
| append | 9 | worker (signer, writable) · Market · record (writable) · raw `SourceSpecV1` · raw `ProviderReleaseV1` · raw `RelayerKeySetV1` · raw `RelayedAdapterConfigV1` · Instructions sysvar · Clock sysvar |
| seal | 7 | worker (signer, writable) · Market · record (writable) · raw `ProviderReleaseV1` · raw `RelayerKeySetV1` · Instructions sysvar · Clock sysvar |
| retire | 4 | worker (signer, writable) · Market (writable) · record (writable) · RentCredit (writable) |

The resolution that consumes a sealed record uses the existing Source
acceptance frame with an **empty** provider payload plus the readonly sealed
record, which is structurally the same shape the shared-observation profile
already uses (`AcceptEvidence` with an empty extension plus an accepted child).
Signatures are verified at append and seal time, never at resolve time; the
record's program ownership and PDA derivation are the authority thereafter.
This is the same pattern the Pyth extension already relies on for a
receiver-owned `PriceUpdateV2`.


### 4.5 Release binding: the relayer key set *is* the provider release

`ProviderReleaseV1` already commits five identities and needs no change:

| Field | Binding for this family |
| --- | --- |
| `provider_family_id` | `RelayedMainnetStateV1` |
| `adapter_release_id` | the on-devnet dClutch adapter ELF release |
| `provider_deployment_release_id` | the content ID of the immutable `RelayerKeySetV1` record |
| `decoding_rules_id` | the `RelayedDecodingRulesV1` record (§4.6) |
| `transport_profile_id` | `RelayedTerminalOneTransaction` or `RelayedObservationRecord` |

The third row is the whole design. For Pyth, `provider_deployment_release_id`
names an on-chain deployment; here it names a key set. Both are immutable
content IDs, and both are already the thing `SourceMaterialV2` reaches through
`SourceSpecV1.provider_release_id`. Consequences, all of which fall out rather
than being added:

- **The key set cannot rotate inside a market's life.** Rotation means a new
  `RelayerKeySetV1`, hence a new `ProviderReleaseV1`, hence a new
  `SourceSpecV1`, hence a new `SourceMaterialV2`, hence a new Market generation.
  That is the existing immutability discipline applied unchanged, and it is the
  correct answer: a market's truth-carrier must not change under the holders.
- **Disclosure is structural.** The key set is reachable by content ID from the
  Market identity, so the operator and frontend can render it without a
  side-channel. `AGENTS.md` requires "an honest user-visible status"; for this
  family that status must name, at founding and on the market page: the family,
  the `n` keys and the threshold `m`, the pinned mainnet program identity, and
  the Product's named failure outcome — in the plain form *"this market's truth
  about Solana mainnet is carried by these keys; if they sign falsely it
  resolves falsely; if they go silent it resolves to <failure outcome>."*
- **Heterogeneous recovery is already expressible.** `RecoveryAttemptV1` carries
  a per-attempt `source_spec_id` **and** `provider_release_id`
  (verified-from-source, `crates/dclutch-source-contract/src/lib.rs:1558`). A
  market may therefore name primary = relayer key set A, recovery 0 = a
  disjoint relayer key set B, recovery 1 = a Pyth release, with no new
  machinery. §4.8 uses this.

### 4.6 Cross-cluster program identity, and the Loopscale defense

The Loopscale failure was trusting an account's *shape* without binding the
program that owns it. Cross-cluster it has a second face, and this lane
measured it rather than reasoning about it.

**Verified-on-chain (§8, reads 3–6):** the Meteora DBC program
`dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN` is deployed on **both**
mainnet-beta and devnet, and its `Program` account data is **byte-identical on
the two clusters** — the same 36 bytes, the same Loader-V3 tag 2, the same
`programdata_address` `HUfnSSiJxgspQm6C1rkqv6L3XgVtn7AESApgCQpCXCYh`. The
`ProgramData` accounts are not:

| | mainnet-beta | devnet |
| --- | --- | --- |
| `ProgramData` space | 2,326,622 | 1,983,613 |
| `deployment_slot` | 423,941,138 | 464,763,515 |
| `upgrade_authority` | `JADaUV8kvDpDbJr55wxXJHVaBS3VCj8thZZHjfeuCVLd` | `DHLXnJdACTY83yKwnUkeoDjqi4QBbsYGa1v8tJL76ViX` |
| upgradeable | yes (tag 1) | yes (tag 1) |

So a devnet observation of the venue's `Program` account is
**indistinguishable** from a mainnet one. Program identity alone does not
identify a cluster. This is the cross-cluster Loopscale variant, and it is why
`observed_cluster_id` is a signed field rather than an adapter assumption: it
converts an otherwise-invisible substitution into an explicit, non-repudiable
false statement by a named key.

The full defense stack, every layer of which is an existing primitive:

1. `observed_cluster_id` must equal the release-pinned mainnet genesis hash
   `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d` (verified-on-chain, read 1).
2. `account_set_id` pins the exact ordered pubkey list. This family does **not**
   support "whatever pool the caller names" — the pool address is fixed at
   founding, so the caller-supplied-account attack is closed at the wire.
3. Every attested account's `owner` is compared against the pinned expected
   owner for its position. Right shape, wrong owner refuses.
4. The venue `Program` and `ProgramData` ride in the set, and the adapter
   reconstructs a `DeploymentObservationV1` from
   (`program`, `program_owner`, `program_executable`) = attested account 1,
   (`programdata`, `programdata_owner`, `programdata_executable`,
   `deployment_slot`, `upgrade_authority`) = attested account 2's 45-byte
   inline prefix through the existing Loader-V3 parse,
   `programdata_link` = account 1's decoded 36-byte body,
   `elf_digest` = account 2's `tail_digest`,
   and runs `ArtifactReleaseV1::authenticate_deployment` **unchanged**, which
   compares every one of those by exact equality.

Upgrade policy is **P-B** from `CHAIN_STATE_SOURCES_2026_08.md` §6.2 —
pinned digest, and a venue upgrade drives the Source to the Product's named
failure outcome. The measurement above makes that a live risk rather than a
hypothetical: DBC's mainnet `upgrade_authority` is present and non-null today,
so a routine venue upgrade during a market's life is a thing that will happen.
The Product must state the failure outcome at founding.

`RelayedDecodingRulesV1` is the family's layout grammar record. Per position it
must name: the expected owning program pubkey; the discriminator, with the
explicit statement that a discriminator is **not** a layout version; the
admitted `data_len` set; the pinned `inline_len`; the field offsets valid at
each admitted length; sentinel semantics; the fixed-point scale; and the derived
`Observation { atoms, unix_seconds }` with one named `RoundingBoundary`. This
is `CHAIN_STATE_SOURCES_2026_08.md` §6.3's list, made a content-addressed
record. Nothing in it is relayer-visible.

One consequence worth stating because it is easy to get backwards: `inline_len`
is pinned **per position in the decoding rules**, not chosen per submission. A
relayer that inlines a different prefix produces a different `account_set_id`
and its attestation is refused before any byte of `data` is read.

### 4.7 Time, staleness, and the two clocks

There are two clocks and they belong to different clusters. The Source contract
already mixes them, so the family cannot dodge the question by construction; it
has to bound it.

**What the contract actually does** (verified-from-source,
`crates/dclutch-source-contract/src/lib.rs:3579` and `:3586`):

```text
primary_deadline(window) = window.end_unix_seconds + window.max_age_seconds
require_after(current, deadline)  <=>  current > deadline        (strict)
```

and `current` is `current_unix_seconds`, the **devnet** `Clock` the SBF adapter
supplies to every Source transition. So `WindowSpecV1.end_unix_seconds`
participates in a devnet-clock comparison whatever else it is used for.

**The resolution.** `WindowSpecV1`'s `start_unix_seconds` and
`end_unix_seconds` are **wall-clock times**, not any cluster's times. Both
clusters' `Clock.unix_timestamp` are validator-timestamp-oracle estimates of
wall clock (verified-from-source, `solana-clock`), so they are comparable to
each other to within a skew that neither protocol bounds. Given that:

- The `Observation.unix_seconds` fed to the window and the statistic is
  `unix_timestamp` decoded from the **attested mainnet `Clock` sysvar** account
  (5 fields, `slot` / `epoch_start_timestamp` / `epoch` /
  `leader_schedule_epoch` / `unix_timestamp`, 40 bytes, verified-from-source).
  The proposition is about mainnet, so the observation's timestamp is mainnet's.
- `primary_deadline` and every `RecoveryAttemptV1.deadline_unix_seconds` are
  compared against the devnet `Clock`, unchanged. They are liveness deadlines,
  not claim semantics, and devnet time is the right clock for "has this market
  waited long enough."
- `RelayedAdapterConfigV1.max_observation_age_seconds` is the staleness bound
  and it spans the two:
  `devnet_now - attested_mainnet_time <= max_observation_age_seconds`.

**`max_age_seconds` is therefore doing double duty**, and that is worth stating
rather than discovering. Its first job is the liveness grace after a window
closes. Its second, in this family only, is to absorb the devnet-to-mainnet
clock skew, because a positive skew shortens the real time an attestation has
to arrive. `RelayedAdapterConfigV1.max_cluster_skew_seconds` exists so the
allowance is named per-family and separately checkable instead of hiding inside
a window field whose primary job is something else, and it yields one
founding-time admission predicate:

```text
window.max_age_seconds >= config.max_cluster_skew_seconds
```

so that skew alone can never trigger the funded failure walk.

`max_cluster_skew_seconds` is **provisional**. Its lifting plan is a
measurement the first slice must produce: sample both clusters'
`Clock.unix_timestamp` over a bounded period and report the maximum observed
`|devnet_now - mainnet_now|`.

**Withhold-and-replay.** `WindowSpecV1.max_future_skew_seconds` guards a
*future* timestamp, and the relayer cannot forge mainnet time — it is attested
account data. What it can do is observe at slot `s`, hold the attestation, and
submit it much later. `max_observation_age_seconds` is the only defense against
that, and it must be set tightly. It is a bound, not an assumption of relayer
promptness.

**Cadence.** `StatisticKind::OddScheduledMedian` requires strict equal cadence,
which `CHAIN_STATE_SOURCES_2026_08.md` §6.4 already flags as a hazard under
congestion. Relaying makes it strictly worse: the relayer reads at *finalized*
commitment — roughly 32 slots, about 13 s at 400 ms slots (*chain-derived
approximation*) — and then needs a devnet slot to land in. The first relayed
slice therefore uses `WindowKind::Terminal` + `StatisticKind::TerminalSample`,
which the graduation product needs anyway. A
relayed scheduled-median profile waits for the cadence-tolerance lift already
queued in the dossier. This is a sequencing decision, not a limitation of the
transport.

### 4.8 Liveness: what happens when the relayer goes silent

The existing Source recovery design already covers this completely. Verified
against `crates/dclutch-source-contract/DESIGN.md` and
`crates/dclutch-resolution-codec/src/lib.rs`, the walk is:

1. Market founds with `SourceMaterialV2` naming primary
   `SourceSpecV1` → `ProviderReleaseV1` R0 (relayer key set K0), and an ordered
   `RecoveryPolicyV1` whose attempts carry `{source_spec_id,
   provider_release_id, deadline_unix_seconds, funding_allocation_id}`.
2. The relayer goes silent; no acceptance arrives before
   `primary_deadline = window.end_unix_seconds + window.max_age_seconds`
   (§4.7).
3. **Any** permissionless worker submits `FundedTransitionActionV3::FailNext`.
   The pure transition checks the material binding and the persisted
   generation, requires `devnet_now > primary_deadline` strictly, and requires
   the **next** attempt's `funding_allocation_id` to equal one
   adapter-authenticated `FundingState` allocation; the worker is credited
   `work_paid` and receives a `FundedTransitionReceiptV1` carrying `worker`,
   `work_paid`, `funding_remaining`. Phase → `Recovery(0)`
   (verified-from-source, `fail_next_view`).
   Each later leg advances the same way against **its own**
   `deadline_unix_seconds`, and `fail_next` refuses rather than exhausting when
   no successor exists (`Error::RecoveryNotExhausted`).
4. Because `RecoveryAttemptV1` carries its own `provider_release_id`, recovery
   leg 0 may name a **different trust root** — a disjoint relayer key set, or a
   Pyth release for a product that has a price analogue. Relayer liveness
   failure therefore degrades to a named alternative source, not to a stuck
   market.
5. If every active leg expires: `Exhaust`, then `CommitFailure` — both
   permissionless and funded — and `CommitFailure` commits the **Product's own**
   failure selector out of `FiniteResultMapV1`. A caller never selects a result.
6. `RetireResolution` closes the state into the pre-existing RentCredit
   beneficiary with the exact Market child-count delta.

The property this buys, stated precisely: **a silent relayer cannot make a
market unresolvable. It can only drive the market to a pre-disclosed outcome,
along a bounded, prepaid, permissionless path that pays whoever walks it.**

That imposes one founding-time admission predicate, which is arithmetic rather
than judgement. With `MAX_RECOVERY_ATTEMPTS = 4`, the worst-case walk is four
`FailNext` plus one `Exhaust` plus one `CommitFailure` = **six** funded
transitions, so a market using a relayed source must prepay a bounty
compartment covering six `work_paid` credits before it may open. Hoard
principal cannot fund it (`AGENTS.md`); the capability contract owns the amount;
this is the existing "deferred physical creation must be precommitted and
prepaid" rule with a concrete count attached.

**Equivocation is structurally bounded too.** The record PDA is seeded by
`(market, generation, account_set_id, observed_slot)`, so at most one record can
exist per set per slot. A relayer that signs two different observations of the
same set at the same slot cannot overwrite the first; it can only publish a
second signed message that contradicts an on-chain one, which is a permanent,
publicly checkable proof of equivocation.

Filling versus sealing splits the same way and it is deliberate: **filling is
1-of-n authenticated, sealing is m-of-n.** A single honest member can complete a
record; the quorum only certifies it. A malicious member who fills a record with
false bytes cannot get it sealed, and the honest members simply build a record
at a different slot — so a bad fill is a wasted rent deposit and a permanent
signed lie, not a denial of service.

### 4.9 The trust surface, in one paragraph

The relayer can **lie** and it can **withhold**. Nothing else. It cannot move
principal, mint or burn claims, change a Market's identity, alter the decoding
rules, resolve to a caller-chosen outcome, or make a market unresolvable: those
are owned by the Hoard, the supply ledger, the immutable Market identity, the
`decoding_rules_id` record, `FiniteResultMapV1`, and the funded permissionless
failure path respectively, none of which the relayer touches. Withholding is
bounded by §4.8 and costs the relayer nothing and gains it nothing beyond
forcing a pre-disclosed failure outcome. Lying is unbounded in effect — a
1-of-1 key that signs false account bytes moves the market to whatever outcome
it wants — and is mitigated only by three things, all of which must be true and
stated: the key set is disclosed in the Market's own identity and cannot rotate
under the holders; every attestation is a non-repudiable signed statement that
anyone with a mainnet RPC endpoint can falsify permanently, after the fact; and
the threshold `m` is a release parameter, so raising it from 1 is a re-founding
rather than a redesign. This is the accepted cost of v1, and it must be written
in the Product, not discovered at resolution.

### 4.10 The swap path, made checkable

"Swapping trust roots never moves semantics" is only worth saying if it is
falsifiable. It is:

| Trust root | `provider_family_id` | `provider_deployment_release_id` | `transport_profile_id` | `decoding_rules_id` |
| --- | --- | --- | --- | --- |
| PoA 1-of-1 | `RelayedMainnetStateV1` | key set, n=1, m=1 | record | **X** |
| Multi-relayer m-of-n | `RelayedMainnetStateV1` | key set, n=5, m=3 | record | **X** |
| TEE-attested signer | `RelayedMainnetStateV1` | key set + attestation policy | record | **X** |
| Wormhole Queries (**no occupant today**, §3) | `QueriedMainnetStateV1` | guardian set identity | query | **X** |

The rightmost column is the assertion: `decoding_rules_id` is the **same 32
bytes** in every row. Every layout fact, offset, sentinel, scale, and rounding
boundary is carried by that content ID, and the trust root is carried by the
other two. A release test can assert the equality directly. If a future
transport requires a different `decoding_rules_id`, the family has leaked
semantics into transport and the design has failed — that is the tripwire.

Moving from row 1 to row 2 additionally requires the m-of-n seal path of §4.4
(one short seal message per signer), which is why it is specified in v1 even
though v1 sets m = 1.

### 4.11 The daemon

A small Rust service. Specification only; nothing here is authorized to run.

**Boundaries.**

- Depends on a Solana RPC client, the wire codec crate, and an Ed25519 signer.
  **Its dependency closure must contain no venue IDL, SDK, or layout crate**
  (§4.1). This is a checkable release property.
- The attestation signing key and the devnet fee-payer key are **distinct**.
  The fee payer is hot and replaceable; the attestation key is the release
  identity and, in the hardened profile, lives behind a separate process or
  HSM boundary that receives message bytes and returns signatures.
- It holds no market policy, no thresholds, and no schedule beyond "which
  account sets to observe, how often."

**Observation loop, per watched `account_set_id`.**

1. One `getMultipleAccounts` at `commitment: finalized` covering the entire set.
   Per-account `getAccountInfo` calls are **forbidden**: the batch call returns
   a single `context.slot` for the whole response, which is the RPC-side
   analogue of the operator invariant
   `dclutch-provider-transport-v3-operator::require_same_finalized_observation`
   (verified-from-source). Mixed-slot account sets are the observation bug this
   family most needs to not have.
2. The venue `ProgramData` account is fetched with
   `dataSlice { offset: 0, length: 45 }` in that same batch. Its full tail
   digest is expensive — 2.3 MB for DBC today (verified-on-chain, read 5) —
   but it changes only on redeploy, and `deployment_slot` sits in the 45-byte
   prefix the daemon already fetches every cycle. The daemon therefore caches
   the tail digest keyed by `(programdata_pubkey, deployment_slot)` and
   recomputes only when the observed slot changes. A redeploy is always
   detected from the prefix; a stale cached digest is unreachable.
3. Build each `RelayedAccountObservationV1` body, verify `inline_len` against
   the pinned rules, sign one `RelayedMainnetAccountAttestationV1` per account
   and one `RelayedObservationSetSealV1` per set.
4. Submit: Ed25519 precompile instruction immediately followed by the dClutch
   instruction, v0, over the Market's ALT. On blockhash expiry, **re-sign the
   transaction, never re-observe** — the attestation is bound to a slot and
   re-observing silently changes the fact being attested.

**Publication is a requirement, not a nicety.** For every message it signs, the
daemon must publish the exact message bytes and the mainnet slot to a public
location. This is the entire mitigation for "the relayer can lie" (§4.9): an
attestation nobody can check against mainnet is a trust assumption; one that is
published is a falsifiable claim. A relayer profile without publication should
not be released.

**Failure behaviour.** On RPC disagreement, a missing account, an unexpected
`data_len` outside the admitted set, or a `deployment_slot` change, the daemon
**stops attesting that set** and emits a diagnostic. It never attests a partial
or repaired observation. The market's own funded failure path (§4.8) is the
correct handling of a stopped relayer, and it is better than any repair the
daemon could invent.

---

## 5. Products mapped onto transports

| Product | What the claim is *about* | Transport | What blocks it |
| --- | --- | --- | --- |
| SOL/USD range and tail protection | the price of a major asset | **T-1**, Pyth devnet, upgraded generation | nothing structural; needs the §2.5 staleness re-measurement and a release pinned to the post-cutover generation |
| BTC/ETH/USDC equivalents | same | **T-1** | confirm each feed is in the sponsored set on devnet and re-measure its cadence separately |
| Meteora DBC graduation on mainnet | a specific mainnet account's discrete state | **T-3**, record profile, `Terminal` + `TerminalSample` | the family itself (§6) |
| pump.fun graduation on mainnet | same | **T-3**, a *second* `decoding_rules_id` under the same `provider_family_id` | lands after DBC; requires disclosing that the pinned ELF binds an unpublished artifact |
| Longtail pool price thresholds | mainnet pool reserves | **T-3** plus Mechanism B (scheduled median, funded submissions) | the `OddScheduledMedian` cadence-tolerance lift, worsened by finalized-commitment relay latency (§4.7) |
| any of the mainnet-account rows | — | **T-2** — **unavailable** | the devnet guardian set is one test key with quorum 1, and the guardian sets do not match (§3.2) |

The first row is the one that matters for the demo's breadth and it costs
nothing new. The third row is the one that proves the capability, and it is the
only one this design has to build.

---

## 6. The smallest first slice

### 6.1 The recommendation

**Build the Meteora DBC mainnet-graduation market, resolved on devnet through
`RelayedMainnetStateV1` in the record profile.**

`CHAIN_STATE_SOURCES_2026_08.md` §8.1 already ranks this first on venue
grounds — published program source, `const_assert_eq!(PoolState::INIT_SPACE,
416)` pinning the layout in source, an explicit four-state `MigrationProgress`
enum, no in-place account growth, no fixed-point scale, no window, no
statistic. Adding the relay transport does not weaken that ranking; it
strengthens it, because a **discrete latched observable is the one class that
tolerates minutes of relay latency without any statistical apparatus**. A price
product would have to answer "how stale is too stale" before it could answer
anything else; a graduation product does not.

The account set is the four of §4.2. The observable is
`MigrationProgress`/`is_migrated`, latched on first authenticated observation
per the dossier's irreversibility caveat, mapped through `FiniteResultMapV1`.

### 6.2 Gate ordering inside the slice

Three gates, in this order, because each isolates one failure class:

1. **Transport gate.** A one-account set containing only the mainnet `Clock`
   sysvar. Exercises the wire, the record PDA, the append and seal routes, the
   Ed25519 adjacency parse, the window join, and the two-clock staleness rule —
   with zero venue decoding and zero program-identity binding. It is a bad
   product and a good gate; it must not ship as a Market.
2. **Venue gate.** The full four-account set, the pinned `ArtifactReleaseV1`
   for the mainnet DBC deployment, and byte-exact `PoolState` decoding.
3. **Failure gate.** The relayer stops. The funded permissionless walk of §4.8
   executes end to end with exact three-ledger closure.

### 6.3 U-009-shaped evidence the slice must produce

The dossier's §8.2 list applies unchanged. This transport adds five items and
sharpens two.

1. **Real ABI, no mock authority.** `PoolState` fixtures at exactly 416 bytes
   derived from the published source, labeled synthetic observations per O-007.
2. **Real cross-cluster program-identity binding.** A `DeploymentObservationV1`
   reconstructed from an attestation of the **mainnet** program pair and
   authenticated against a founding-time pin, with `upgrade_authority`
   compared. Paired with its negative: the *byte-identical devnet* `Program`
   account measured in §4.6 must refuse, and it must refuse **on
   `observed_cluster_id`**, because nothing else distinguishes the two. This is
   an executed test, not a remark.
3. **Upgrade-mid-market refusal, executed.** P-B: pinned `elf_digest` no longer
   matches, observation refuses, `Primary → Recovery → Exhausted →
   FailureCommitted`, Product's named failure outcome, exact closure.
4. **Hostile-decode refusal corpus**, extended for this transport: wrong
   `observed_cluster_id`; `account_set_id` mismatch; `inline_len` other than the
   pinned value; `tail_digest` mismatch; a fully-inline body whose
   `tail_digest` is not the empty-string digest; a seal signed by a key outside
   `RelayerKeySetV1`; a seal count below `seal_threshold`; a second attestation
   of the same set at the same slot (refused by PDA uniqueness — and the test
   must show the refusal *preserves* the first record); two appends carrying
   different `observed_slot`; an Ed25519 instruction that is not immediately
   preceding; a descriptor whose message offset points outside the current
   instruction's data; an all-zero signature.
5. **Release-bound adapter.** One `ProviderReleaseV1`, one `RelayerKeySetV1`,
   one `RelayedDecodingRulesV1`, one transport profile. No parallel path
   (O-005), no mock fallback (U-009).
6. **Capacity and cost evidence.** Measured `SourceCapacityProfileV1`; real CU,
   packet, and rent figures for append, seal, and resolve. Two numbers are
   already derivable and must be checked against measurement rather than
   assumed: the record at 4,792 bytes is rent-exempt at
   `(128 + 4792) × 3480 × 2 = 34,243,200` lamports = **0.0342432 SOL**
   (*mathematical*, from `solana-rent`'s
   `DEFAULT_LAMPORTS_PER_BYTE_YEAR = 3480`, `DEFAULT_EXEMPTION_THRESHOLD = 2.0`,
   `ACCOUNT_STORAGE_OVERHEAD = 128`, verified-from-source); and the prepaid
   failure bounty must cover **six** funded transitions (§4.8).
7. **Vocabulary discipline**, sharpened. Nothing in this family is an oracle, a
   price feed, or a TWAP. And the standing rule that devnet execution is not
   mainnet evidence acquires a specific new edge here: a devnet market resolving
   a mainnet claim produces **devnet evidence about a mainnet proposition**.
   The correct sentence is "the devnet market accepted an attestation asserting
   mainnet state," never "the market observed mainnet."
8. **Relayer independence, checked mechanically.** The daemon crate's resolved
   dependency closure contains no venue IDL, SDK, or layout crate (§4.1).
9. **Cross-cluster clock skew, measured.** Sample both clusters'
   `Clock.unix_timestamp` over a bounded period; report the maximum observed
   `|devnet_now − mainnet_now|`; use it to justify
   `max_cluster_skew_seconds` and retire its *provisional* label.
10. **The swap tripwire, demonstrated.** Two `ProviderReleaseV1` records with
    different `provider_deployment_release_id` (two disjoint relayer key sets)
    and **byte-identical `decoding_rules_id`**, both resolving the same
    Product to the same outcome from the same attested bytes (§4.10).

### 6.4 What the slice deliberately does not include

Large-account chunking (an account whose release-pinned inline window exceeds
448 bytes, needing a persisted incremental SHA-256 midstate across
transactions); m-of-n with m > 1; the scheduled-median relayed profile; a
Realm-level shared relayed-observation cache that would let N markets on the
same pool share one record. Each is a named lift with a stated trigger, not an
oversight. The v1 record is a direct Market child, so N markets on one pool pay
N rent deposits today; that is the cost of not inventing a second ontology
before the first one has executed.

---

## 7. What could not be verified

Listed explicitly rather than filled in with plausible values.

1. **Meteora DBC `PoolState` field offsets.** `CHAIN_STATE_SOURCES_2026_08.md`
   §2.6 names the fields (`sqrt_price`, `base_reserve`, `quote_reserve`,
   `is_migrated`, `migration_progress`, `finish_curve_timestamp`,
   `volatility_tracker`) and the source-pinned total of 416 bytes, but not the
   offsets. `RelayedDecodingRulesV1` cannot be minted until they are read from
   `dynamic-bonding-curve` source. Nothing in this document invents them.
2. **Whether the graduation fields are prefix-contiguous.** Moot for the first
   slice, which carries all 416 bytes inline, but it decides whether a future
   venue can use the one-transaction profile with a partial `inline_len`.
3. **Devnet↔mainnet `Clock` skew.** No measurement exists. §4.7's
   `max_cluster_skew_seconds` is *provisional* with the measurement as its
   lifting plan.
4. **Devnet Pyth cadence beyond one window.** The 310–318 s figure is
   *measured-profile* over twelve consecutive updates in a single observation
   window. It is not a bound, and no maximum-gap measurement exists.
5. **Whether devnet sponsored feeds other than SOL/USD share that cadence.**
   Only SOL/USD was sampled; BTC, ETH and USDC accounts were confirmed to exist
   and decode, not to be cranked at any particular rate.
6. **Whether a free or devnet-scoped Pyth Hermes tier exists.** Both documented
   endpoints returned HTTP 401 unauthenticated. No devnet endpoint is
   documented.
7. **Whether Pyth published a formal sunset date for the legacy pyth-client v2
   price accounts.** The on-chain stop is pinned at **2024-08-29T21:44:01Z**
   (devnet SOL/USD `J83w4HKf…`, verified-on-chain; its BTC/USD sibling stopped
   two seconds later, so this was a clean shutdown rather than decay). No
   announcement page stating a date was found.
8. **The §4.4 packet arithmetic against a real frame.** The 425-byte fixed
   overhead and the resulting `MAX_RELAYED_INLINE_BYTES_V1 = 448` are derived
   from the wire formats, not measured against a constructed `ProgramTest` or
   devnet transaction. The first slice must re-derive them from a real frame
   and the constant must move if the measurement disagrees.
9. **Wormhole Queries mainnet `sol_account` quorum success rate**, and Queries
   pricing. Both require an API key.
10. **Whether the production Wormhole testnet CCQ guardian has its Solana
    watcher enabled** for devnet account reads. Solana does not appear in the
    documented testnet supported-networks table at all.
11. **Loader version coverage.** DBC is Loader V3 on both clusters
    (verified-on-chain). The family's program-identity path assumes Loader V3
    throughout — the 45-byte metadata offset, the `ProgramDataV3View` parse, the
    tag-0-retains-stale-authority quirk. A venue that migrates to Loader V4
    needs a distinct observation profile, and no such profile exists here.
12. **The governance behind the mainnet DBC upgrade authority**
    `JADaUV8kvDpDbJr55wxXJHVaBS3VCj8thZZHjfeuCVLd` — whether it is a multisig,
    a timelock, or a single key was not read. It bears directly on how often
    P-B will fire.
13. **Whether the venue state accounts of interest can be read at a single
    finalized context slot together with a 2.3 MB `ProgramData` account** within
    one `getMultipleAccounts` response on public RPC endpoints. §4.11 assumes a
    `dataSlice` makes this cheap; the response-size and rate limits of public
    endpoints were not tested.

---

## 8. RPC read log

All reads are read-only public JSON-RPC. No writes, no signing, no keypairs, no
airdrops. Reads 1–6 were taken directly by this lane; reads 7–30 and 31–36
were taken by two delegated verification agents under the same constraints and
are reproduced here with their targets.

| # | Cluster | Method | Target | Result used in |
| --- | --- | --- | --- | --- |
| 1 | mainnet-beta | `getGenesisHash` | — | `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d` (§4.6) |
| 2 | devnet | `getGenesisHash` | — | `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` (§4.6) |
| 3 | mainnet-beta | `getAccountInfo` | `dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN` | 36 B, tag 2, PD `HUfnSSiJ…` (§4.6) |
| 4 | devnet | `getAccountInfo` | same | **byte-identical** 36 B (§4.6) |
| 5 | mainnet-beta | `getAccountInfo` dataSlice(0,45) | `HUfnSSiJxgspQm6C1rkqv6L3XgVtn7AESApgCQpCXCYh` | space 2,326,622; slot 423,941,138; auth `JADaUV8k…` (§4.6) |
| 6 | devnet | `getAccountInfo` dataSlice(0,45) | same | space 1,983,613; slot 464,763,515; auth `DHLXnJdA…` (§4.6) |
| 7–8 | both | `getMultipleAccounts` | `rec5EK…`, `HDwcJB…`, `gSbePeb…` | legacy generation; `gSbePeb…` **absent on mainnet** (§2.2) |
| 9–10 | both | `getMultipleAccounts` dataSlice(0,45) | legacy `ProgramData` accounts | divergent deploy slots and sizes (§2.2) |
| 11–14 | mixed | `getBlockTime` | 487855452, 487852630, 441901675, 327047637 | cutover timestamps (§2.2) |
| 15–16 | both | `getMultipleAccounts` | legacy push oracle + `7UVimffx…` | legacy SOL/USD, `Full` (§2.2) |
| 17–18 | both | `getMultipleAccounts` | Wormhole `GuardianSet[0..5]` + bridge config | GS0 only, 5 keys, identical (§2.4) |
| 19 | devnet | `getMultipleAccounts` dataSlice(0,240) | `J83w4HKf…`, `HovQMDrb…` | legacy v2 frozen 2024-08-29 (§2.2) |
| 20–21 | both | `getMultipleAccounts` | `HDw2E7…`, `rec2HH…`, `pyt2F4…`, `7AviUf…`, `CJHmJw4F…` | upgraded generation live, prices agree (§2.3) |
| 22–23 | both | `getMultipleAccounts` dataSlice(0,45) | upgraded `ProgramData` ×3 | **identical sizes across clusters** (§2.2) |
| 24–25 | both | `getMultipleAccounts` | receiver `Config` PDAs ×4 | 1 data source, Pythnet emitter, fee 0, minsig 3 (§2.3) |
| 26–29 | both | `getSignaturesForAddress`, `getTransaction` | `7AviUf…`, `7UVimf…` | cadence and pushers (§2.5) |
| 30 | devnet | `getMultipleAccounts` | legacy pyth v2 program + PD | devnet-only deployment (§2.2) |
| 31 | mainnet-beta | `getMultipleAccounts` dataSlice(0,64) | `worm2ZoG…`, `2yVjuQwp…` | core bridge, `guardian_set_index = 7` (§3.2) |
| 32 | devnet | `getMultipleAccounts` dataSlice(0,64) | `worm2ZoG…`, `3u8hJUVT…`, `6bi4JGDo…` | `worm2ZoG…` **not a program on devnet**; index 0 (§3.2) |
| 33 | mainnet-beta | `getAccountInfo` | `6YLGQQEweF82hbPSWCSeJqifWyT8Pm4QXa3mWSLwjYSh` | 396 B, 19 keys (§3.2) |
| 34 | devnet | `getAccountInfo` | `dxZtypiKT5D9LYzdPxjvSZER9MgYfeRVU5qpMTMTRs4` | **36 B, 1 key** (§3.2) |
| 35–36 | both | `getMultipleAccounts` dataSlice(0,36) | `EFaNWErqAtVWufdNb7yofSHHfWFos843DFpu4JBw24at` | shim deployed on both (§3.6) |

Non-Solana HTTP probes, liveness only:
`hermes.pyth.network/v2/updates/price/latest` → **401**;
`pyth.dourolabs.app/hermes/v2/updates/price/latest` → **401**;
`query.wormhole.com/v1/query` and `testnet.query.wormhole.com/v1/query` → 405
(POST-only, endpoint exists).

---

## 9. Sources

First-party repository, verified-from-source in a local checkout:

- `crates/dclutch-source-contract/DESIGN.md` and `src/lib.rs`
  (`ProviderReleaseV1`, `SourceSpecV1`, `SourceAccessProfile`, `WindowSpecV1`,
  `RecoveryAttemptV1`, `SourceMaterialV2`)
- `crates/dclutch-registry-contract/src/artifact.rs`
  (`DeploymentObservationV1`, `ArtifactReleaseV1::authenticate_deployment`)
- `crates/dclutch-registry-svm/src/lib.rs`
  (`LOADER_V3_PROGRAM_BYTES = 36`,
  `LOADER_V3_PROGRAMDATA_METADATA_BYTES = 45`, `ProgramDataV3View`)
- `crates/dclutch-direct-contract/src/adapter.rs`
  (`inspect_preceding_ed25519_batch_item_v2`, `ED25519_DESCRIPTOR_BYTES = 14`,
  `SOLANA_PACKET_DATA_SIZE_3_0 = 1_232`)
- `crates/dclutch-resolution-codec/src/lib.rs`
  (`FundedTransitionActionV3`, `FundedTransitionReceiptV1`)
- `crates/dclutch-provider-transport-v3-operator/src/lib.rs`
  (`require_same_finalized_observation`)
- `docs/research/CHAIN_STATE_SOURCES_2026_08.md`,
  `docs/evidence/PYTH_SYNTHETIC_RELEASE_V1.md`,
  `docs/compost/PYTH_LOCAL_UPGRADED_2026_08_22.md`, `docs/OMISSION_INDEX.md`

Solana runtime, verified-from-source in the local crate registry:

- `agave-precompiles-4.3.0-beta.2/src/{lib,ed25519}.rs` — precompiles verify
  top-level instructions during transaction verification; not CPI-reachable
- `solana-cost-model-4.3.0-beta.2/src/block_cost_limits.rs` —
  `ED25519_VERIFY_STRICT_COST = 2_400`, `SIGNATURE_COST = 720`
- `solana-cost-model-4.3.0-beta.2/src/cost_model.rs::get_signature_cost`
- `solana-ed25519-program-3.0.0/src/lib.rs` — descriptor layout and sizes
- `solana-rent-3.1.0/src/lib.rs` — `DEFAULT_LAMPORTS_PER_BYTE_YEAR = 3480`,
  `DEFAULT_EXEMPTION_THRESHOLD = 2.0`, `ACCOUNT_STORAGE_OVERHEAD = 128`
- `solana-clock-*/src/lib.rs` — `Clock` field set
- `solana-loader-v3-interface-*/src/state.rs` — `UpgradeableLoaderState`

Pyth, verified-from-docs:

- https://docs.pyth.network/price-feeds/core/contract-addresses/solana
- https://docs.pyth.network/price-feeds/core/upgrade/contracts
- https://docs.pyth.network/price-feeds/core/push-feeds/solana
- https://docs.pyth.network/price-feeds/core/api-reference
- https://forum.pyth.network/t/ongoing-op-pip-131-pyth-core-deprecation-svm/2671
- https://www.pyth.network/blog/the-pyth-core-upgrade

Wormhole, verified-from-source at `wormhole-foundation/wormhole`
`f42dea010390518bc5825f65b3f0f57765f62f23` (2026-08-25):

- `node/pkg/query/{request,response}.go`, `node/cmd/ccq/{permissions,p2p,http}.go`
- `node/pkg/watchers/solana/ccq.go`, `sdk/vaa/quorum.go`
- `solana/bridge/program/src/{api/verify_signature.rs,accounts/guardian_set.rs}`
- `svm/wormhole-core-shims/programs/verify-vaa/`,
  `svm/wormhole-core-shims/crates/definitions/src/solana.rs`
- `whitepapers/0013_ccq.md`
- https://wormhole.com/docs/products/queries/reference/supported-networks/
- https://github.com/anza-xyz/agave `program-runtime/src/execution_budget.rs`
  (`secp256k1_recover_cost = 25_000`)
