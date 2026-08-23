# Production payoff compiler transport v1

This is the JSON seam between Glass and operatord's pure Rust CLI/loopback
adapter. It does not define compiler math. The adapter calls
`compile_production_payoff_v1`, encodes its canonical artifacts through their
Rust codecs, calls `assemble_compiled_product_series_bundle_v1`, and serializes
the result below. Glass treats every result as an untrusted proposal.

Start the same-origin static client and compiler endpoint with an explicit
compiler build/release digest:

```text
operatord compiler-serve --compiler-release-sha256 HASH [--port N] [--static DIR]
```

Or pipe the request through the identical pure implementation:

```text
operatord compile-payoff --compiler-release-sha256 HASH < request.json
```

Neither command reads RPC, a wallet, or a browser session; neither signs,
submits, registers, or persists anything.

## Exact definition

Every integer is a canonical unsigned decimal string. Every rational is reduced
and encoded as:

```json
{ "numerator": "-1", "denominator": "3" }
```

The common envelope is:

```json
{
  "schema": "dragons-clutch/compiler/production-payoff-definition/v1",
  "productTermsId": "<32-byte lowercase hex>",
  "kind": "exact-categorical | exact-smooth | analytic-smooth",
  "definition": {}
}
```

`exact-categorical` has `coordinateDomainMin`, `coordinateDomainMax`, `knots`,
`cellPayouts`, `ambiguityPolicyRegistryValue`, and
`edgePolicyRegistryValue`. `cellPayouts` is a rectangular array of rationals;
`knots` has exactly one fewer entry than payout rows. Rust remains responsible
for domain ordering, simplex sums, least-denominator integerization, and every
semantic refusal.

`exact-smooth` has `basis`, `controlValues`, and `maximumLiability`.
`analytic-smooth` has `basis` and `shape`. A smooth `basis` contains:

```json
{
  "degree": "1",
  "coordinateDomainMin": "0",
  "coordinateDomainMax": "100",
  "payoutDenominator": "1000000",
  "knots": ["0", "100"],
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

## Untrusted proposal

The adapter canonicalizes the validated definition exactly as Glass does:
object keys sorted recursively, array order retained, compact JSON, UTF-8. It
places SHA-256 of those bytes in `inputCanonicalSha256`.

```json
{
  "schema": "dragons-clutch/compiler/production-payoff-proposal/v1",
  "authority": "untrusted-compiler-proposal",
  "registrationAuthority": false,
  "compilerReleaseSha256": "<32-byte lowercase hex>",
  "inputCanonicalSha256": "<32-byte lowercase hex>",
  "productTermsId": "<same ID as definition>",
  "classification": "exact-categorical | exact-smooth | analytic-smooth",
  "spanStatus": "exact-in-span | certified-approximation",
  "nativeClaimBasis": {
    "id": "<typed content ID as lowercase hex>",
    "bytesHex": "<exactly 2352 canonical bytes>"
  },
  "certificate": null,
  "bounds": [],
  "subdivisionDepth": null,
  "compiledProductSeriesBundle": {
    "id": "<typed bundle ID as lowercase hex>",
    "bytesHex": "<exactly 528 canonical bytes>",
    "identities": {}
  }
}
```

Categorical evidence uses `certificate: null`, no bounds, and null subdivision
depth. Exact smooth evidence requires nonempty certificate `id`/`bytesHex`, no
bounds, and null depth. Analytic evidence requires the certificate, a decimal
`subdivisionDepth`, and all eight rational metrics in this exact vocabulary:

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

Each bound is `{"name":"...","value":{"numerator":"...","denominator":"..."}}`.
Only analytic requests may be `certified-approximation`.

`identities` must contain exactly the sixteen `CompiledProductSeriesBundleV1`
fields, in camel case:

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

Glass joins `nativeClaimBasisId` to the basis proposal and
`capabilityProfileId` to the selected release. It does not recompute typed IDs
from bytes; doing so in JavaScript would create a second semantic owner. Onchain
registration must reopen the registry, Source release, all canonical artifacts,
and the bundle and recompute every ID and binding.

## Compiler request

Both transports accept one request no larger than 327,680 bytes. All fields are
closed: unknown fields are refused. The compiler release is configured on the
process command line; `expectedCompilerReleaseSha256` is only a fail-closed
join to that configuration, never a caller-selected assertion.

```json
{
  "schema": "dragons-clutch/compiler/production-payoff-request/v1",
  "expectedCompilerReleaseSha256": "<configured 32-byte lowercase hex>",
  "definition": {},
  "bundleInputs": {
    "registryCapabilityProfileV2BytesHex": "<800 canonical bytes>",
    "sourceReleaseManifestId": "<32-byte lowercase hex>",
    "evidenceOnlyRecoveryPolicyV1BytesHex": "<208 canonical bytes>",
    "productTemplateV4BytesHex": "<256 canonical bytes>",
    "priceMeasurePolicyV1BytesHex": "<96 canonical bytes>",
    "marketGenesisProfileV2BytesHex": "<416 canonical bytes>",
    "seriesFundingQuoteV1BytesHex": "<280 canonical bytes>",
    "seriesAttachmentPlanV1BytesHex": "<112 canonical bytes>",
    "seriesPlanV5BytesHex": "<152 canonical bytes>",
    "seriesFundingTermsV2BytesHex": "<240 canonical bytes>"
  }
}
```

`definition` is the complete definition envelope documented above. Only its
recursively key-sorted, compact, normalized UTF-8 JSON is hashed into
`inputCanonicalSha256`; outer request fields do not affect that digest. Every
fixed body is hostile-decoded by its one Rust codec. The basis is deliberately
absent from `bundleInputs`: it comes only from the payoff compiler, and the
canonical assembler refuses supplied artifacts that name a different basis.

The HTTP route is exactly `POST /v1/compiler/production-payoff`. It binds only
on IPv4 loopback, requires an exact loopback `Host`, requires
`application/json`, rejects transfer encoding and oversized bodies, and accepts
either no `Origin` (CLI clients) or exactly `http://{Host}`. It emits no wildcard
CORS policy. Consequently a browser uses it from the Glass files served by the
same `compiler-serve` origin; static files can still be hosted elsewhere for
read-only inspection and CLI-proposal import.
