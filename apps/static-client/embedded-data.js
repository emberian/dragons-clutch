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
    },
    "displaySnapshot": {
      "schema": "dragon-clutch.glass-display-snapshot.v1",
      "schemaVersion": 1,
      "snapshotIdentity": {
        "reviewedTreeCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
        "releaseBinding": "unbound-offline-snapshot",
        "releaseSourceCommit": null
      },
      "termsBinding": {
        "termsVersion": "terms-v0-offline-sample-r2",
        "digest": "sha256:62b06b2107636686648507e4f9ecd8a4d90733dcebf81177d4a63b25bc698d02"
      },
      "evidence": [
        {
          "id": "local-terms-r2",
          "kind": "LOCAL_FIXTURE",
          "subject": {
            "id": "terms-r2",
            "label": "Bundled terms fixture r2"
          },
          "scope": "The bundled canonical terms object is locally inspectable and its declared digest can be recomputed in a secure browser context.",
          "sourceRef": {
            "repositoryPath": "apps/static-client/terms.json",
            "locator": "terms-v0-offline-sample-r2"
          },
          "negativeBoundary": "It is a display fixture, not live market terms, an offer, a Terms account, or chain state.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "file-sha256",
              "value": "1eeaef6b7d08a032d9c05adff112e2bb92e7f7eeb51b6948656a6a679be1a6b1",
              "path": "apps/static-client/terms.json"
            }
          }
        },
        {
          "id": "native-v1-degree1-fixture",
          "kind": "LOCAL_FIXTURE",
          "subject": {
            "id": "native-v1-degree1",
            "label": "Native V1 degree-one fixture"
          },
          "scope": "A Rust-generated native basis and shape-certificate fixture is available for dependency-free local inspection.",
          "sourceRef": {
            "repositoryPath": "docs/implementation/NATIVE_BSPLINE_CLIENT_SCHEMA_V1.md",
            "locator": "Cross-language fixture and checks"
          },
          "negativeBoundary": "It is a generated sample, not an onchain Terms account, certificate account, market, or release artifact.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "file-sha256",
              "value": "9192facd76eb5bc0affdd58ee6b8f55a1b08e78765f45116b2b69adbdf39a886",
              "path": "research/bspline-shape-compiler/fixtures/native-v1-degree1.json"
            }
          }
        },
        {
          "id": "native-client-fixture-gates",
          "kind": "HOST_TESTED",
          "subject": {
            "id": "native-client-parser",
            "label": "Offline native fixture gates"
          },
          "scope": "Node tests check local native fixture agreement, canonical structure, digest parsing, and unsigned preview construction.",
          "sourceRef": {
            "repositoryPath": "apps/static-client/test/native-bspline-v1.mjs",
            "locator": "native offline fixture gates"
          },
          "negativeBoundary": "The browser check does not recompile the Rust compiler, validate full runtime policy, submit, deploy, or authenticate an account.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "source-revision",
              "value": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
              "path": "apps/static-client/test/native-bspline-v1.mjs"
            }
          }
        },
        {
          "id": "bspline-lean-model",
          "kind": "PROVED_MODEL",
          "subject": {
            "id": "native-basis-model",
            "label": "Native degree one through three B-spline model"
          },
          "scope": "The committed Lean model checks named clamped rational constructions, exact partition properties, largest-remainder selection, and related model properties.",
          "sourceRef": {
            "repositoryPath": "CURRENT_TRUTH.md",
            "locator": "Native claim semantics"
          },
          "negativeBoundary": "It does not refine Rust source, parsing, accounts, CPI, SBF code generation, or runtime behavior.",
          "identity": {
            "sourceCommit": "8c929a93c4b530744a24431c2b2cd9fca067c1bb",
            "artifact": {
              "kind": "source-revision",
              "value": "8c929a93c4b530744a24431c2b2cd9fca067c1bb",
              "path": "lean/DragonsClutch/BSpline.lean"
            }
          }
        },
        {
          "id": "bspline-finite-bridge",
          "kind": "CHECKED_FINITE",
          "subject": {
            "id": "native-basis-bridge",
            "label": "Native B-spline finite bridge"
          },
          "scope": "Digest-pinned Lean-computed fixtures match the production evaluator on a named finite corpus, and named actual-source mutations go red.",
          "sourceRef": {
            "repositoryPath": "CURRENT_TRUTH.md",
            "locator": "Native claim semantics"
          },
          "negativeBoundary": "Finite agreement does not prove all Rust behavior, parser/refusal order, overflow behavior, compiler, SBF, or runtime.",
          "identity": {
            "sourceCommit": "be8eba3815bf27e79f845d6aed006d77dfb899ef",
            "artifact": {
              "kind": "source-revision",
              "value": "be8eba3815bf27e79f845d6aed006d77dfb899ef",
              "path": "crates/clutch-bspline/src/lib.rs"
            }
          }
        },
        {
          "id": "native-point-resolution",
          "kind": "SBF_EXECUTED",
          "subject": {
            "id": "native-point-resolution",
            "label": "Focused native point resolution and exits"
          },
          "scope": "Focused local-bank evidence covers degree-one through degree-three point resolution, native vector persistence and replay, and exact-lot internal and bearer exits.",
          "sourceRef": {
            "repositoryPath": "CURRENT_TRUTH.md",
            "locator": "Native claim semantics"
          },
          "negativeBoundary": "It is not a clean release, public deployment, production source-ingestion path, or general native settlement claim.",
          "identity": {
            "sourceCommit": "ae2e155c6fa5b067592bb32fd638cf976679865d",
            "artifact": {
              "kind": "elf-sha256",
              "value": "e448f1a9a5fe7c80b2d8ece939dab059ef64ccadab11fa5952328cd31ed35a32",
              "path": "docs/implementation/NATIVE_RESOLUTION_SBF.md"
            }
          }
        },
        {
          "id": "client-browser-boundary",
          "kind": "UNAVAILABLE",
          "subject": {
            "id": "glass-browser-surface",
            "label": "Browser chain authority"
          },
          "scope": "This static client has no RPC, wallet connection, account discovery, serializer, signing, submission, or background work.",
          "sourceRef": {
            "repositoryPath": "apps/static-client/README.md",
            "locator": "Glass static client"
          },
          "negativeBoundary": "A local static projection is not protocol truth and cannot establish account state or execute a transition.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "source-revision",
              "value": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
              "path": "apps/static-client"
            }
          }
        },
        {
          "id": "release-chain-binding",
          "kind": "UNAVAILABLE",
          "subject": {
            "id": "release-chain-binding",
            "label": "Release chain binding"
          },
          "scope": "The manifest names no program ID, checked ELF, bundle digest, immutable CID, deployment manifest, or official client.",
          "sourceRef": {
            "repositoryPath": "apps/static-client/manifest.json",
            "locator": "releaseIdentity"
          },
          "negativeBoundary": "The unpublished manifest cannot imply a deployment, an official endpoint, or release authenticity.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "unavailable",
              "value": "not-available",
              "path": null
            }
          }
        },
        {
          "id": "release-evidence-stop",
          "kind": "STOP",
          "subject": {
            "id": "release-evidence",
            "label": "Release evidence"
          },
          "scope": "A clean joined baseline, checked release manifest, independent rebuild, audit, and public-network deployment are absent.",
          "sourceRef": {
            "repositoryPath": "CURRENT_TRUTH.md",
            "locator": "Artifact release evidence"
          },
          "negativeBoundary": "No focused run, local proof, static page, or manifest placeholder closes a release gate.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "unavailable",
              "value": "not-available",
              "path": null
            }
          }
        },
        {
          "id": "source-ingestion-stop",
          "kind": "STOP",
          "subject": {
            "id": "source-ingestion",
            "label": "Production source ingestion"
          },
          "scope": "The default source registry has no admitted production provider/parser release.",
          "sourceRef": {
            "repositoryPath": "docs/implementation/AUTHENTICATED_SOURCE_CONSTRUCTION_V1.md",
            "locator": "Exact STOPs"
          },
          "negativeBoundary": "Mock-source lifecycle evidence cannot authenticate a real provider, parser, source account, or production feed.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "unavailable",
              "value": "not-available",
              "path": null
            }
          }
        },
        {
          "id": "active-native-mode-stop",
          "kind": "STOP",
          "subject": {
            "id": "active-native-mode",
            "label": "Active native mode binding"
          },
          "scope": "Native point resolution and exact exits have focused evidence, but active lifecycle seams still need immutable Terms-selected mode binding.",
          "sourceRef": {
            "repositoryPath": "CURRENT_TRUTH.md",
            "locator": "Non-negotiable STOP ledger"
          },
          "negativeBoundary": "Native resolution evidence must not be used to describe every Split, Merge, materialize, or dematerialize path as mode-safe.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "unavailable",
              "value": "not-available",
              "path": null
            }
          }
        },
        {
          "id": "liveness-kernel",
          "kind": "HOST_TESTED",
          "subject": {
            "id": "prepaid-liveness-kernel",
            "label": "Prepaid liveness arithmetic"
          },
          "scope": "The pure kernel separates market work, storage, resolution, per-order clear, and per-order settle compartments even when fees are zero.",
          "sourceRef": {
            "repositoryPath": "docs/implementation/LIVENESS_ADMISSION_KERNEL.md",
            "locator": "Prepaid liveness admission kernel"
          },
          "negativeBoundary": "No account codec, authenticated funding route, measured maximum, neutral-failure recipient, SBF path, or inclusion guarantee exists.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "source-revision",
              "value": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
              "path": "crates/clutch-liveness/src/lib.rs"
            }
          }
        },
        {
          "id": "unaccepted-worktree",
          "kind": "IN_FLIGHT",
          "subject": {
            "id": "unaccepted-worktree",
            "label": "Unaccepted worktree material"
          },
          "scope": "ResolutionWork and Direct Selection V3 are described only as unaccepted design material in the active worktree.",
          "sourceRef": {
            "repositoryPath": "CURRENT_TRUTH.md",
            "locator": "Snapshot boundary"
          },
          "negativeBoundary": "They are excluded from the accepted baseline and create no frontend capability, ABI, runtime route, selection, or settlement claim.",
          "identity": {
            "sourceCommit": "ef32495b6b97f6f5c5212e84dedd3cacd217b2a7",
            "artifact": {
              "kind": "unaccepted-worktree",
              "value": "not-identity-bound",
              "path": null
            }
          }
        }
      ],
      "lifecycle": [
        {
          "id": "offline-fixture-inspection",
          "label": "Offline fixture inspection",
          "statement": "Bundled canonical terms, manifest records, and local evidence labels can be inspected without a network connection.",
          "evidenceRefs": [
            "local-terms-r2",
            "client-browser-boundary"
          ],
          "prerequisiteIds": [],
          "boundaryRefs": [
            "browser-authority",
            "release-binding"
          ],
          "disposition": "inspect-only",
          "action": "local-preview"
        },
        {
          "id": "native-byte-preview",
          "label": "Native byte preview",
          "statement": "The shipped offline SDK can inspect canonical native Terms and shape-certificate bytes and construct unsigned preview data.",
          "evidenceRefs": [
            "native-v1-degree1-fixture",
            "native-client-fixture-gates"
          ],
          "prerequisiteIds": [
            "offline-fixture-inspection"
          ],
          "boundaryRefs": [
            "native-semantic-boundary",
            "browser-authority"
          ],
          "disposition": "inspect-only",
          "action": "local-preview"
        },
        {
          "id": "chain-binding",
          "label": "Release and chain binding",
          "statement": "There is no program ID or checked deployment identity for this client to validate.",
          "evidenceRefs": [
            "release-chain-binding",
            "release-evidence-stop"
          ],
          "prerequisiteIds": [],
          "boundaryRefs": [
            "release-binding"
          ],
          "disposition": "not-released",
          "action": "none"
        },
        {
          "id": "source-ingestion",
          "label": "Production source ingestion",
          "statement": "No admitted provider/parser release can construct a production authenticated source history.",
          "evidenceRefs": [
            "source-ingestion-stop"
          ],
          "prerequisiteIds": [
            "chain-binding"
          ],
          "boundaryRefs": [
            "source-boundary"
          ],
          "disposition": "blocked",
          "action": "none"
        },
        {
          "id": "settlement",
          "label": "Full coupled settlement",
          "statement": "Candidate selection, entitlement construction, lapse, refund, and terminal settlement remain incomplete as one lifecycle.",
          "evidenceRefs": [
            "release-evidence-stop"
          ],
          "prerequisiteIds": [
            "chain-binding",
            "source-ingestion"
          ],
          "boundaryRefs": [
            "settlement-boundary"
          ],
          "disposition": "blocked",
          "action": "none"
        }
      ],
      "basisComparison": [
        {
          "id": "native-degree-zero",
          "aspect": "Degree 0 · categorical native Eggs",
          "nativeDegreeZero": "An exhaustive, disjoint, ordered partition of the admitted state domain.",
          "nativeSmoothDegreesOneToThree": "Not the smooth construction; exactly one native Egg receives full weight at an admitted realized state.",
          "categoricalCompatibilityLowering": "This is native categorical semantics, not a proxy for degrees one through three.",
          "boundaryRefs": [
            "native-semantic-boundary"
          ]
        },
        {
          "id": "native-smooth-degrees-one-to-three",
          "aspect": "Degrees 1–3 · native smooth Eggs",
          "nativeDegreeZero": "Not a one-hot outcome model; neighboring native Eggs may overlap around a resolved point.",
          "nativeSmoothDegreesOneToThree": "Open-clamped B-splines with overlapping local support, nonnegative exact weights, and partition of unity.",
          "categoricalCompatibilityLowering": "Lowering is a separately disclosed adapter with an error statement when inexact; it never renames or redefines the smooth product.",
          "boundaryRefs": [
            "native-semantic-boundary"
          ]
        }
      ],
      "localFixtures": [
        {
          "id": "terms-fixture-record",
          "label": "Bundled terms fixture r2",
          "localPath": "terms.json",
          "fileSha256": "1eeaef6b7d08a032d9c05adff112e2bb92e7f7eeb51b6948656a6a679be1a6b1",
          "producer": "Reviewed static-client terms input",
          "evidenceRefs": [
            "local-terms-r2"
          ],
          "notChainState": true,
          "provenanceBoundary": "The fixture digest binds canonicalTerms only; it is not a released Terms account, live market, or offer."
        },
        {
          "id": "native-fixture-record",
          "label": "Native V1 degree-one fixture",
          "localPath": "research/bspline-shape-compiler/fixtures/native-v1-degree1.json",
          "fileSha256": "9192facd76eb5bc0affdd58ee6b8f55a1b08e78765f45116b2b69adbdf39a886",
          "producer": "Rust-generated cross-language fixture",
          "evidenceRefs": [
            "native-v1-degree1-fixture",
            "native-client-fixture-gates"
          ],
          "notChainState": true,
          "provenanceBoundary": "The native preview remains unsigned and offline; Rust recompilation and onchain policy admission remain outside it."
        }
      ],
      "boundaries": [
        {
          "id": "browser-authority",
          "title": "Browser authority",
          "category": "trust",
          "text": "No RPC, wallet, transaction, or background capability exists in this client.",
          "evidenceRefs": [
            "client-browser-boundary"
          ]
        },
        {
          "id": "native-semantic-boundary",
          "title": "Native claim semantics",
          "category": "semantic",
          "text": "Smooth degree-one through degree-three claims must never be silently lowered into degree-zero categories.",
          "evidenceRefs": [
            "bspline-lean-model",
            "bspline-finite-bridge"
          ]
        },
        {
          "id": "release-binding",
          "title": "Release and deployment",
          "category": "release",
          "text": "No checked release manifest, program ID, ELF identity, public deployment, or official hosted client exists.",
          "evidenceRefs": [
            "release-chain-binding",
            "release-evidence-stop"
          ]
        },
        {
          "id": "source-boundary",
          "title": "Source provenance",
          "category": "lifecycle",
          "text": "Production source creation, parser/provider admission, and Clock-bound history remain stopped; mock evidence is not a provider claim.",
          "evidenceRefs": [
            "source-ingestion-stop",
            "native-point-resolution"
          ]
        },
        {
          "id": "settlement-boundary",
          "title": "Selection and settlement",
          "category": "lifecycle",
          "text": "Narrow direct seams do not constitute an end-to-end candidate selection, entitlement, settlement, lapse, or refund lifecycle.",
          "evidenceRefs": [
            "release-evidence-stop"
          ]
        },
        {
          "id": "active-mode-boundary",
          "title": "Active native mode",
          "category": "semantic",
          "text": "Focused native resolution evidence does not close mode binding across every active lifecycle seam.",
          "evidenceRefs": [
            "active-native-mode-stop",
            "native-point-resolution"
          ]
        },
        {
          "id": "liveness-boundary",
          "title": "Prepaid liveness",
          "category": "lifecycle",
          "text": "Collateral, cash, reservation, rent principal, prepaid work, and donations have separate accounting roles; future fees and Hoard principal never fund required work.",
          "evidenceRefs": [
            "liveness-kernel"
          ]
        },
        {
          "id": "resolution-work-unaccepted",
          "title": "ResolutionWork is unaccepted worktree material",
          "category": "evidence",
          "text": "A resumable Begin, Fold, Finalize, and Abort design is excluded from the accepted baseline and creates no client action or live ABI claim.",
          "evidenceRefs": [
            "unaccepted-worktree"
          ]
        },
        {
          "id": "direct-selection-unaccepted",
          "title": "Direct Selection V3 is unaccepted worktree material",
          "category": "evidence",
          "text": "The staged top-three verification and terminal lapse model is excluded from the accepted baseline and creates no selection or settlement affordance.",
          "evidenceRefs": [
            "unaccepted-worktree"
          ]
        }
      ]
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
