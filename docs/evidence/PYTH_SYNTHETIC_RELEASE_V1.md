# Synthetic-local Pyth release V1

> **Status: partially superseded, 2026-08-27.** Everything below this banner is
> the original 2026-08-22 text, retained verbatim. Two of its claims have since
> been measured against the live clusters and one of them is wrong. Read
> [§Supersession](#supersession-2026-08-27) before using the quorum paragraph or
> deriving any release from this file. The measurements live in
> `fixtures/pyth/upgraded-2026-08-26/PROVENANCE.md`.

This manifest derives the feature-gated dClutch lab release from
`fixtures/pyth/local-upgraded-2026-08-22/`. It is local execution evidence,
not a production catalog row, provider-availability claim, or authenticated
Solana cluster release.

## Measured identifiers

- Provider source commit:
  `f50a3faf9fc5a223a22889799b2f778900f186b3`.
- `pyth-solana-receiver-sdk-2.0.0.crate` SHA-256:
  `245b1b03dd2177402018b6072fcbb7bea5b3d280427b1954796bf1dc189be48b`.
- The archive's extracted `src/price_update.rs` SHA-256, used as the exact
  PriceUpdate codec identifier:
  `12d0ce8bc3907ae2949043397eaf3d5bd25deed98450c6969d957be402c807ae`.
- Receiver ABI identifier, equal to the captured `receiver.so` SHA-256:
  `c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64`.
- Router ABI identifier, equal to the captured `router.so` SHA-256:
  `f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb`.

The adapter authenticates the exact Program and ProgramData keys and their
deployment slots. It does not hash the combined 1,072,824 ELF bytes during
each resolution; the ProgramData generation is the onchain executable bind,
while the ELF digests make the fixture and ABI evidence reproducible.

## Domain-separated identifiers

The synthetic local release label/cluster identifier is SHA-256 of these exact
bytes:

```text
dclutch/synthetic-local-release/v1 || 00 || local-upgraded-2026-08-22
```

Its digest is
`4081d55d4031313fcf4b7c41313d547a9441c8f9c048741a7a951b3e035e22d9`.
This is deliberately not a devnet or mainnet genesis hash.

The dClutch adapter semantic identifier is SHA-256 of these exact byte strings,
separated by one `00` byte:

```text
dclutch/pyth-adapter/v1
resolve-categorical-pyth-v1
internal-post-update
inline-terminal-receipt
```

Its digest is
`3fdfc94589c69b133864468320976f8e790e7fe0f145897b6eabc22bd7c8711b`.

## Quorum distinction

The captured receiver Config has `minimum_signatures = 5`. That local receiver
policy is not the router's guardian-set cardinality or strict-majority fact.
The adapter binds the complete Config digest, while the authenticated router
generation and verified EncodedVaa path own full-VAA verification. The lab
release therefore must not compare the Config value five with the 19-guardian
strict-majority threshold ten.

The fixture feed ID `[0x2a; 32]` has no real asset meaning. A lab Market must
pair it with explicitly synthetic base and quote semantic IDs in its inline
feed profile, and that profile cannot seed a production Market.

---

## Supersession (2026-08-27)

Added by the PY lane after a bounded public-RPC observation of both clusters.
The section above is unchanged; this section says what is now known.

### 1. This is the *upgraded* generation, not a superseded one — CORRECTED

The Pyth Core cutover landed 2026-08-26T16:00:49Z and it was reasonable to
assume that a fixture captured on 2026-08-22 sat on the wrong side of it. It
does not. Measured 2026-08-27:

- `fixtures/pyth/local-upgraded-2026-08-22/receiver.so` is **byte-identical**
  to the live receiver ELF on `mainnet-beta` **and** `devnet`.
- `fixtures/pyth/local-upgraded-2026-08-22/router.so` is **byte-identical** to
  the live Wormhole-receiver ELF on both clusters.
- Its recorded devnet `ProgramData` complete-body digests (`7122abc6…`,
  `f26f4b53…`) and deployment slots (460,336,311 / 460,336,290) still reproduce
  the live devnet accounts exactly.

So the receiver ABI identifier pinned above,
`c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64`, **is** the
upgraded generation's receiver. The 2026-08-22 capture named the upgraded
program IDs and took the upgraded binaries; the cutover made that generation
canonical without changing those bytes.

**Nothing in the ABI moved, and the adapter needed no change.** The
`pyth_price_route` campaign passes unmodified against the new generation,
executing `rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp`'s real ELF.

This corrects the reading in `docs/design/MAINNET_STATE_RELAY.md` §2.2
consequence 1, which called this identity "the **legacy** generation". It is
not. The operative caution in that consequence still stands for a different
reason — see §3 below.

### 2. The quorum-distinction paragraph is superseded

The paragraph above says:

> The lab release therefore must not compare the Config value five with the
> 19-guardian strict-majority threshold ten.

That instruction described a real hazard against the guardian set of the time.
It no longer describes the deployment. Measured on both clusters:

| | lab fixture | live upgraded generation |
| --- | --- | --- |
| guardian set index | 0 | 0 |
| guardian set cardinality | 19 (synthetic, upstream `dummy_guardians_addresses()`) | **5** |
| `Config.minimum_signatures` | 5 | **3** |
| `Config` fee | 1 | **0** |
| admitted data sources | 1, chain 1, emitter `[0x01; 32]` | 1, **chain 26 (Pythnet)**, emitter `PythnetPythnetPythnetPythnetPyth` |
| strict majority of the set | 10 | **3** |

Two things follow.

**The comparison the paragraph forbids no longer exists.** On the live
generation `minimum_signatures = 3` and the strict majority of five is also 3,
so the receiver's own policy value and `PythReleaseV1`'s strict-majority rule
(`count / 2 + 1`) coincide. They did not under the nineteen-key set. The
`InvalidStrictMajority` refusal in `dclutch-pyth-svm` needs no change; a
release for the live generation is `guardian_set_count = 5,
required_guardian_count = 3`, which is exactly what its tests already use.

**The lab's quorum shape is now unlike production, and that must be said out
loud.** The fixture's 5-of-19 is a *lab* shape. It is not a scaled-down model
of the live 3-of-5 and must never be described as one. What the lab proves is
that the real router ELF performs real signature verification and that dClutch
rolls back atomically around it — not that dClutch has exercised the
production quorum.

**The trust root is a 3-of-5 Pyth-controlled multisig**, guardian set index 0,
`expiration_time = 0`, five keys byte-identical across clusters. "Zero new
trust" for a Pyth-sourced market remains exact, but what that trust *is* has
become materially smaller, and that belongs in the Product's disclosure rather
than a footnote.

### 3. A release is per cluster, and the reason is not the binary

`PythReleaseV1` already carries a `cluster_id`. This is the measured
justification for requiring it to be nonzero and cluster-specific, and for
naming the cluster by its **genesis hash** rather than letting an adapter infer
it:

| cluster | genesis hash |
| --- | --- |
| `mainnet-beta` | `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d` |
| `devnet` | `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` |

The upgraded generation's three ELFs are identical across clusters. What is
**not** identical, and what a release actually binds:

| bound fact | shared? |
| --- | --- |
| receiver / router / push-oracle ELF digest | shared |
| program ids and `ProgramData` keys | shared |
| `ProgramData` allocation sizes | shared |
| **deployment slot** | **per cluster** |
| **upgrade authority** | **per cluster** (`6oXTdojy…` vs `upg8KLAL…`) |
| **complete `ProgramData` body digest** | **per cluster** |
| **`Config.governance_authority`** | **per cluster** (`6oXTdojy…` vs `7g4Los4W…`) |
| **complete `Config` digest** | **per cluster** |
| **`GuardianSet[0]` account digest** | **per cluster** (`creation_time` differs by 104 s) |
| guardian key material, `Config` trust fields | shared |

So `config_digest`, `receiver_deployment_slot` and `router_deployment_slot` in
`PythReleaseV1Input` are cluster-specific by construction, and two release rows
are required to cover both clusters even though one ABI serves both. This is
the operative caution the "legacy generation" reading was reaching for: no
production release row may be derived from this file **for either cluster**,
because this file pins a lab `Config` (`05038cf7…`) that exists on neither.

`crates/dclutch-svm-harness/tests/pyth_upgraded_generation.rs` makes this
executable: it rebuilds all six observed `ProgramData` bodies from one shared
copy of each ELF plus six committed per-cluster headers, and requires each
observed digest exactly.

### 4. Staleness is a measurement with a date, not a constant

*measured-profile, 2026-08-27*, `getSignaturesForAddress` on the SOL/USD
account `7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE`, successful transactions
only, distinct block times:

| | `devnet` | `mainnet-beta` |
| --- | --- | --- |
| window | 170.42 h (7.10 d) | 12.21 h |
| gaps | 1,997 | 7,017 |
| p50 / p90 / p99 | 313 / 321 / 325 s | 7 / 9 / 13 s |
| **max observed gap** | **4,784 s** | **21 s** |

The single largest devnet gap ran 2026-08-25T08:42:02Z → 10:01:46Z; the next
largest was 354 s, so it was one discrete outage rather than drift.

This supersedes the earlier n = 12 single-window figure of 310–318 s. That
sample had the median right (p50 = 313 s here) and **missed the tail
entirely**. The 400 s devnet staleness budget it suggested would have refused
every read for 79 consecutive minutes two days ago. A `WindowSpecV1`
`max_age_seconds` is a maximum, not a median.

Both figures remain **measured-profile and provisional**. A finite observation
window is not a bound; the lifting plan is continuous observation with a
running maximum, not a longer one-off measurement. Until that exists, a devnet
majors Product must treat provider silence as an expected state with a funded
permissionless failure path, not as an anomaly.

Push identity, most recent 12 postings per cluster: devnet is a single payer
(`4p16wya1Vw2u9w22oah4yXQgySb6eWKRRLMsEXCreish`, 12/12); mainnet-beta is **not
a single payer** (`9F6ApEtzkHVdZXzsury6BYmyEh4pahDBxuhNLaGC6saC` 9/12,
`9uFDvq24JQ8SzbFuQ5opBDfNy2NoCxUJCHdSapBxLufF` 3/12). This corrects an earlier
"sole fee payer" reading for mainnet. It is a sample of 12, not an enumeration
of the pusher set.

### 5. Which surface targets which generation

| surface | generation | evidence level |
| --- | --- | --- |
| `fixtures/pyth/local-upgraded-2026-08-22/` | upgraded (ELFs), lab (Config, guardians, VAA, price) | synthetic-local execution of the real upgraded ABI |
| `fixtures/pyth/upgraded-2026-08-26/` | upgraded, as deployed | dated cluster observation; not executable on its own |
| `crates/dclutch-pyth-svm` decoders | generation-independent byte views | unit; now exercised against both lab and live shapes |
| `crates/dclutch-pyth-svm::release::PRODUCTION_RELEASES` | none | deliberately empty; no row exists for either cluster |
| `crates/dclutch-svm-harness/tests/pyth_price_route.rs` | upgraded ELFs, lab Config/VAA/price | synthetic-local execution, passes on the new generation |
| `crates/dclutch-svm-harness/tests/pyth_upgraded_generation.rs` | upgraded, as deployed | offline byte evidence against the observation |
| `tools/local-validator/bootstrap/` | upgraded ELFs, lab everything else | local-validator execution |

None of these is devnet-price, provider-availability, production-release, or
mainnet evidence.
