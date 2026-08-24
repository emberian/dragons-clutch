# Product exact-market compiler transport v2

This is the sole current JSON seam between Glass and operatord's pure Rust
Product compiler. It transports an exact payoff definition, the complete
ProfileV4/BundleV6 input graph, and an optional bounded exact-market search. It
does not define compiler math and it does not create registration authority.

Start the same-origin static client and loopback compiler endpoint with an
explicit compiler build/release digest:

```text
operatord compiler-serve --compiler-release-sha256 HASH [--port N] [--static DIR]
```

Or pipe one request through the identical pure implementation:

```text
operatord compile-product-exact-market --compiler-release-sha256 HASH < request.json
```

Neither command reads RPC, a wallet, or a browser session; neither signs,
submits, registers, or persists anything. The configured compiler hash is a
fail-closed transport join, not a measurement of the running binary and not a
checked release manifest.

## Request

Both transports accept one closed-schema request no larger than 327,680 bytes.
Unknown fields, JSON numbers, noncanonical decimal strings, invalid fixed body
widths, zero semantic identities, and malformed addresses are refused.

```json
{
  "schema": "dragons-clutch/compiler/product-exact-market-request/v2",
  "expectedCompilerReleaseSha256": "<configured 32-byte lowercase hex>",
  "programId": "<canonical nonzero Product program address>",
  "definition": {},
  "bundleInputs": {
    "registryProgramReleaseV2BytesHex": "<160 canonical bytes>",
    "registryCapabilityProfileV4BytesHex": "<816 canonical bytes>",
    "sourceReleaseManifestId": "<nonzero 32-byte lowercase hex>",
    "evidenceOnlyRecoveryPolicyV1BytesHex": "<208 canonical bytes>",
    "productTemplateV4BytesHex": "<256 canonical bytes>",
    "priceMeasurePolicyV1BytesHex": "<96 canonical bytes>",
    "marketGenesisProfileV2BytesHex": "<416 canonical bytes>",
    "seriesFundingQuoteV5BytesHex": "<600 canonical bytes>",
    "seriesAttachmentPlanV5BytesHex": "<112 canonical bytes>",
    "seriesPlanV5BytesHex": "<152 canonical bytes>",
    "seriesFundingTermsV2BytesHex": "<240 canonical bytes>"
  },
  "exactMarketSearch": {
    "marketId": "<nonzero 32-byte lowercase hex>",
    "priceId": "<nonzero 32-byte lowercase hex>",
    "prices": ["250000", "750000"],
    "coordinates": ["0", "1", "2"],
    "maximumSubsetEvaluationsPerSupport": "10000"
  }
}
```

`exactMarketSearch` is optional. If it is absent, the proposal still contains
the `exactMarket` key with a null value. Active prices contain 1 through 16
exact `u64` integers. Coordinates contain 1 through 64 strictly increasing
exact `u128` integers. The Rust semantic owner checks the Product outcome width,
the payout-denominator simplex, Terms domain, basis profile, and deterministic
work budget.

The browser obtains `programId` from the acquired checked-release projection;
it is not a free-form compiler field. Operatord decodes it as an exact canonical
Solana address, refuses the all-zero/default address, and requires its 32 bytes
to equal `RegistryProgramReleaseV2.program`. This join happens before any
BundleV6 artifact PDA is derived.

Every fixed body is hostile-decoded by its owning Rust codec. Operatord:

- recomputes the RegistryProgramReleaseV2 identity and requires ProfileV4 to
  name it;
- projects the current capability rules only from ProfileV4;
- compiles the native basis from `definition`, never from a caller-supplied
  basis body;
- requires the definition's Product Terms identity to equal the supplied
  MarketGenesisProfileV2 identity;
- reopens the SeriesPlanV5, FundingTermsV2, QuoteV5, and AttachmentV5 joins;
  and
- assembles the sole current `CompiledProductSeriesBundleV6` graph. The graph
  owns exactly 47 foundation slots: core 0..13, HoardCollateralVault 14,
  OutcomeMint 15..30, and OutcomeCustody 31..46.

The proposed Source release is represented here only by its nonzero manifest
identity. The offline compiler cannot authenticate chain accounts or loader
state. Registration must supply and authenticate the exact Source release body
named by that identity.

## Exact payoff definition

Every integer is a canonical decimal string. Every rational is reduced and
encoded as:

```json
{ "numerator": "-1", "denominator": "3" }
```

The common definition envelope is:

```json
{
  "schema": "dragons-clutch/compiler/production-payoff-definition/v1",
  "productTermsId": "<MarketGenesisProfileV2 identity as lowercase hex>",
  "kind": "exact-categorical | exact-smooth | analytic-smooth",
  "definition": {}
}
```

`exact-categorical` contains `coordinateDomainMin`, `coordinateDomainMax`,
`knots`, `cellPayouts`, `ambiguityPolicyRegistryValue`, and
`edgePolicyRegistryValue`. `cellPayouts` is a rectangular array of exact
rationals and `knots` has exactly one fewer entry than payout rows.

`exact-smooth` contains `basis`, `controlValues`, and `maximumLiability`.
`analytic-smooth` contains `basis` and `shape`. A smooth `basis` contains:

```json
{
  "degree": "2",
  "coordinateDomainMin": "0",
  "coordinateDomainMax": "100",
  "payoutDenominator": "1000000",
  "knots": ["0", "50", "100"],
  "resolvedEdgePolicy": "clamp",
  "ambiguityPolicyRegistryValue": "1",
  "edgePolicyRegistryValue": "1"
}
```

The bounded analytic shape kinds and exact fields are:

| `kind` | fields |
| --- | --- |
| `hard-range` | `low`, `high`, `height` |
| `upper-tail` / `lower-tail` | `strike`, `height` |
| `triangle` | `left`, `peak`, `right`, `height` |
| `capped-call` / `capped-put` | `low`, `high`, `height` |
| `gaussian` | `center`, `sigma`, `height` |

Rust remains responsible for domain ordering, exact simplex sums,
least-denominator integerization, spline compilation, error certification, and
every semantic refusal.

## Untrusted proposal

Operatord canonicalizes the validated definition exactly as Glass does: object
keys sorted recursively, array order retained, compact JSON, and UTF-8. It
places SHA-256 of those bytes in `inputCanonicalSha256`. It canonicalizes the
complete validated request in the same way and places that SHA-256 in
`requestCanonicalSha256`.

The response has this closed top-level shape:

```json
{
  "schema": "dragons-clutch/compiler/product-exact-market-proposal/v2",
  "authority": "untrusted-compiler-proposal",
  "registrationAuthority": false,
  "compilerReleaseSha256": "<configured 32-byte lowercase hex>",
  "programId": "<same Product program as the request and ReleaseV2>",
  "requestCanonicalSha256": "<SHA-256 of the complete canonical request>",
  "inputCanonicalSha256": "<SHA-256 of the canonical definition>",
  "productTermsId": "<same identity as the definition and Genesis>",
  "classification": "exact-categorical | exact-smooth | analytic-smooth",
  "spanStatus": "exact-in-span | certified-approximation",
  "nativeClaimBasis": {
    "id": "<typed content identity>",
    "bytesHex": "<2352 canonical bytes>"
  },
  "certificate": null,
  "bounds": [],
  "subdivisionDepth": null,
  "compiledProductSeriesBundleV6": {
    "id": "<typed BundleV6 identity>",
    "bytesHex": "<528 canonical bytes>",
    "artifact": {
      "kind": "63",
      "context": "<64 zeroes>",
      "exactBodyBytes": "528",
      "programId": "<same Product program>",
      "pda": "<derived content-addressed account>",
      "bump": "<canonical u8 decimal>"
    },
    "identities": {}
  },
  "exactMarket": null
}
```

Categorical evidence has no separate certificate, bounds, or subdivision
depth. Exact smooth evidence requires its recompilable certificate and no
bounds. Analytic evidence requires a certificate, an exact subdivision depth,
and these eight exact rational metrics:

```text
spline-sup-lower
spline-sup-upper
spline-l1-lower
spline-l1-upper
consensus-quantization-sup-upper
consensus-sup-upper
consensus-l1-upper
coefficient-sample-sup-upper
```

`compiledProductSeriesBundleV6.identities` contains exactly:

```text
registryReleaseId
capabilityProfileId
sourceReleaseManifestId
sourcePlaneContractId
sourceSpecId
summaryProgramId
productCompilerReleaseId
nativeClaimBasisId
evidenceOnlyRecoveryPolicyId
productTemplateId
priceMeasurePolicyId
marketGenesisProfileId
fundingQuoteId
attachmentPlanId
seriesPlanId
fundingTermsId
```

The artifact coordinate is exactly the Product artifact seed, kind `63`, and
the recomputed BundleV6 identity under the ReleaseV2-owned Product program.
Global Product artifacts use the all-zero context. The browser requires the
bundle capability-profile identity to equal the acquired checked-release
profile and requires the bundle's Genesis identity to equal `productTermsId`.

## Optional exact-market result

When requested, `exactMarket` contains:

- `authority: "untrusted-compiler-sidecar"` and
  `registrationAuthority: false`;
- `outcome`: `solved`, `unsupported`, `out-of-profile`, or `work-truncated`;
- `coverage`: `full-integer-domain` or `declared-coordinate-subset`;
- `completeFullDomainNegative`, true only for an unsupported search that
  exhausts every support over every integer in the complete Terms domain;
- `claims` with `uniquePrice`, `fairValue`, and `optimalClearing` all exactly
  false;
- exact Market, Product Terms, native basis, price, and BundleV6 bindings;
- exact target prices and payout denominator;
- the coordinate declaration, per-support work counts, exhaustion boundary,
  truncation boundary, and work budget;
- a 1,640-byte canonical work manifest;
- a 544-byte hostile-verifier certificate only for `solved`; and
- a 176-byte sidecar bound to the kind-63, global-context BundleV6 identity.

This search returns an exact certificate found by its deterministic finite
traversal. It does not claim a unique market price, fair value, or optimal
clearing. A declared-coordinate negative is not a complete Product domain
negative, and a work-truncated result is not exhaustion evidence.

## Authority boundary

Every response is a proposal. The browser checks closed transport shapes,
fixed widths, exact request hashes, the configured compiler hash, the selected
program address, the sixteen exposed identities, the native-basis/BundleV6
join, the Genesis/Terms join, and the exact-market request/output bindings. It
does not reinterpret Rust fixed codecs or mint registration capability.

Neither a compiler response, a BundleV6 PDA string, nor an operatord index row
is onchain truth. Registration must reload and authenticate:

- the executing Program and ProgramData loader pair, deployment locus, slot,
  and complete ProgramData/ELF hash;
- RegistryProgramReleaseV2 and RegistryCapabilityProfileV4;
- the exact Source release;
- every content-addressed Product and Series artifact;
- the recomputed BundleV6 body, typed identity, kind-63 PDA, and bump; and
- every exact-market manifest, certificate, and sidecar consumed by an enabled
  onchain route.

The program must recompute all identities and joins from hostile bytes. A
successful offline compile, browser display, fixture, simulation, or devnet
execution is not registration authority or mainnet evidence.

## HTTP boundary

The HTTP route is exactly:

```text
POST /v2/compiler/product-exact-market
```

The pure compiler server binds IPv4 loopback, requires an exact loopback
`Host`, requires `application/json`, rejects transfer encoding and oversized
bodies, and accepts either no `Origin` or exactly `http://{Host}`. It emits no
wildcard CORS policy. A browser can therefore use the endpoint from Glass files
served by the same `compiler-serve` origin. Static files hosted elsewhere remain
usable for read-only inspection and CLI-proposal import.
