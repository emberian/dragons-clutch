/*
 * GENERATED MIRROR — do not hand-edit. Regenerate with `npm run embed`.
 *
 * Verbatim copies of the reviewed manifest.json and terms.json in this
 * directory. They are embedded rather than fetched because the page must run
 * from file:// under a `default-src 'none'` policy that permits no network
 * connection at all. Equality with the reviewed files is enforced by the test
 * `embedded_static_data_equals_reviewed_manifest_and_terms`; a drifted mirror
 * fails `npm test` instead of quietly displaying a different binding.
 *
 * This file contains data only. It has no network, wallet, RPC, signing, or
 * submission capability, and it must never acquire one.
 */
(function (root) {
  "use strict";

  var MANIFEST = {
    "schemaVersion": "dragon-clutch.static-release-manifest.v0",
    "application": {
      "name": "Glass / Dragon's Clutch",
      "version": "0.1.0-offline",
      "releaseStatus": "offline-prototype",
      "official": false
    },
    "releaseIdentity": {
      "sourceRepository": "dragon-clutch",
      "sourceCommit": "UNBOUND-OFFLINE-SNAPSHOT",
      "bundleSha256": "UNPUBLISHED-BUNDLE-DIGEST",
      "ipfsCid": null,
      "githubPagesMirror": null,
      "manifestSha256": "UNPUBLISHED-MANIFEST-DIGEST"
    },
    "clusters": [
      {
        "id": "mainnet-beta",
        "label": "Solana Mainnet Beta",
        "status": "unavailable",
        "rpcPolicy": "user-selected-only",
        "endpoint": null,
        "note": "No RPC endpoint is embedded or contacted by this build."
      },
      {
        "id": "devnet",
        "label": "Solana Devnet",
        "status": "unavailable",
        "rpcPolicy": "user-selected-only",
        "endpoint": null,
        "note": "No devnet deployment is checked by this build."
      },
      {
        "id": "localnet",
        "label": "Local validator",
        "status": "unavailable",
        "rpcPolicy": "user-selected-only",
        "endpoint": null,
        "note": "A local validator is optional future infrastructure, not started here."
      }
    ],
    "programs": [
      {
        "key": "clutch",
        "label": "Dragon's Clutch protocol",
        "programId": null,
        "status": "not-deployed",
        "sbfElfSha256": null,
        "deploymentManifest": null,
        "note": "No program ID or ELF release has been checked."
      }
    ],
    "profiles": [
      {
        "id": "synthetic-six-decimal",
        "label": "Synthetic six-decimal Realm",
        "status": "offline-fixture",
        "mint": "SYNTHETIC-MINT-NOT-ONCHAIN",
        "decimals": 6,
        "termsVersion": "realm-profile-v0",
        "note": "A local shape-only fixture. It has no collateral value and no chain identity."
      },
      {
        "id": "dregg-reference",
        "label": "DREGG reference Realm",
        "status": "reference-only-unchecked",
        "mint": "XkeTXo1125vz5H9svJpGiw4JvLbN8VmMu9cmMvspump",
        "decimals": 6,
        "termsVersion": "realm-profile-v0",
        "note": "Copied from repository prose as a reference only; no deployment or account read is asserted."
      }
    ],
    "terms": {
      "path": "terms.json",
      "termsVersion": "terms-v0-offline-sample-r2",
      "digestAlgorithm": "sha256",
      "digestScope": "canonicalTerms object (sorted keys, compact UTF-8 JSON)",
      "digest": "sha256:62b06b2107636686648507e4f9ecd8a4d90733dcebf81177d4a63b25bc698d02"
    },
    "capabilities": {
      "staticHosting": true,
      "ipfsCompatible": true,
      "rpcReads": false,
      "walletConnection": false,
      "transactionSigning": false,
      "transactionSubmission": false,
      "backgroundWork": false
    }
  };

  var TERMS = {
    "schemaVersion": "dragon-clutch.terms.v0",
    "termsVersion": "terms-v0-offline-sample-r2",
    "canonicalTerms": {
      "collateralProfile": "synthetic-six-decimal",
      "feeAtoms": 0,
      "maxOutcomes": 16,
      "outcomeCount": 2,
      "redemption": "per-outcome-exact-or-refuse-plus-complete-set-exit",
      "rounding": "exact-integer-payout-or-refuse-on-remainder",
      "sourceAdapter": "none-offline-fixture",
      "sourceVersion": "unbound",
      "state": "inspection-only",
      "templateId": "offline-sample-template-v0",
      "window": {
        "endUnixSeconds": 0,
        "startUnixSeconds": 0
      }
    },
    "digest": "sha256:62b06b2107636686648507e4f9ecd8a4d90733dcebf81177d4a63b25bc698d02",
    "semanticsNote": "Redeeming one outcome pays quantity * weight / denominator only when that division is exact; a remainder is refused, never floored or truncated. A holder of a complete set always exits exactly through complete-set redemption. No payout boundary rounds.",
    "warning": "These are display fixtures, not live market terms or an offer. Divisibility policy for fractional payout vectors is not frozen; see docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md."
  };

  root.GlassEmbeddedData = Object.freeze({ manifest: MANIFEST, terms: TERMS });
})(typeof globalThis === "object" ? globalThis : this);
