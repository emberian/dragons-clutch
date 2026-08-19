#!/usr/bin/env python3
"""Generate and check the Dragon's Clutch baseline evidence manifest.

This tool makes "the reviewed offline baseline is intact" a *checkable* claim.
It does not publish, tag, sign, deploy, or release anything, and it attests no
proof content whatsoever. See ``docs/implementation/BASELINE_MANIFEST.md`` for
the schema, the explicit non-attestations, and the promotion path.

Standard library only. Deterministic: for a fixed working tree and a fixed set
of gate outcomes the emitted JSON is byte-identical, except for the fields under
``run`` (wall-clock timestamps), which ``check`` ignores by construction.

Usage
-----
    scripts/baseline_manifest.py emit  [--out PATH] [--allow-dirty] [--run-gates]
    scripts/baseline_manifest.py check [--manifest PATH] [--run-gates]

``emit`` refuses on a dirty working tree unless ``--allow-dirty`` is given; a
manifest emitted with ``--allow-dirty`` carries ``"dirty": true`` and the full
``git status --porcelain`` listing, and is a mid-flight snapshot, never a
baseline.

Exit codes
----------
0   success
1   drift detected (``check``), or a gate outcome contradicted its declaration
2   refusal: dirty working tree under ``--strict`` (the default)
3   environment or usage error
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

SCHEMA = "dragons-clutch/baseline-manifest/v2"
CONTENT_IDENTITY_SCHEMA = "dragons-clutch/tracked-working-tree/v1"
DEFAULT_MANIFEST = "MANIFEST.baseline.json"
DEFAULT_GATE_TIMEOUT = 1800

EXIT_OK = 0
EXIT_DRIFT = 1
EXIT_DIRTY_REFUSAL = 2
EXIT_ENVIRONMENT = 3


# --------------------------------------------------------------------------
# Claim vocabulary (CODEX_HANDOFF.md section 1). Copied verbatim in meaning so
# that a consumer of the manifest never has to guess what a label licenses.
# --------------------------------------------------------------------------

LABEL_VOCABULARY = {
    "IMPLEMENTED": (
        "source exists locally and the named offline checks pass; implies no "
        "formal verification, security review, or deployment readiness; any "
        "runtime evidence must be separately named"
    ),
    "MODEL": (
        "a deterministic reference model, specification, theorem statement, "
        "synthetic experiment, or cost hypothesis exists; not consensus code "
        "and not production evidence"
    ),
    "PROPOSED": (
        "a design choice, parameter, policy, architecture, or backlog item has "
        "not crossed its stated evidence gate"
    ),
    "BLOCKER": (
        "the named work must refuse promotion until it is closed; a blocker is "
        "not permission to weaken the claim or bypass the gate"
    ),
}

NOT_ATTESTED = [
    "no release: nothing here publishes, tags, pushes, or authorizes a release",
    "no signature chain: no signed tag, no signed artifact, no key material, no "
    "transparency log entry",
    "no independent reproducible-build closure: the E0 rlib and deployable SBF "
    "ELF each have a same-machine two-build comparison, but there is no "
    "independent rebuilder, toolchain bootstrap, or dependency-source rebuild",
    "no formal proof content: the Rocq gate typechecks Definitions (zero "
    "theorems) and the root Verus probe is an expected failure; the isolated "
    "Verus batch shadow is in flight and is deliberately absent until its source "
    "and reproduction gate are committed",
    "SBF evidence is local only: the manifest gates a loopback test-validator "
    "differential/lifecycle walk and an in-process Agave bank with Token-2022; "
    "neither is public-cluster, deployment, independent-rebuild, validator-"
    "diversity, or cross-runtime-vector evidence",
    "no SBOM, license closure, fixture provenance chain, or source offer",
    "no production source attestation: the default runtime build refuses Endow "
    "without a registered source release, while the named mock-source path is "
    "local test evidence only",
    "no direct-selection promotion: the measured V2 three-Candidate selection "
    "hits the 1,400,000-CU transaction limit and rolls back; V3 remains a host "
    "model with live ABI/runtime stops",
    "no system-wide terminal/liveness closure: the local ResolutionWork SBF route "
    "does not close the separate production-source, direct-selection, or storage "
    "and retirement stops",
    "no published provenance: the identities below are git object ids. A "
    "configured remote or a pushed branch is neither a signed tag nor a release "
    "artifact, and this manifest asserts nothing about either",
    "no security review and no regulatory closure (Gate L0 remains open)",
]


# --------------------------------------------------------------------------
# Digest inventory. Every entry is derived from a repository path or from a
# declared canonicalization of one. `handoff` records the value CODEX_HANDOFF.md
# section 6 (or a named implementation note) currently prints, so that drift
# between the tree and the prose is itself detectable.
# --------------------------------------------------------------------------

FILE_DIGESTS: list[tuple[str, str, str | None, str | None]] = [
    # (id, path, handoff-declared sha256 or None, handoff reference or None)
    (
        "static_client.terms_json",
        "apps/static-client/terms.json",
        None,
        None,
    ),
    ("static_client.manifest_json", "apps/static-client/manifest.json", None, None),
    ("static_client.app_js", "apps/static-client/app.js", None, None),
    ("static_client.index_html", "apps/static-client/index.html", None, None),
    ("static_client.styles_css", "apps/static-client/styles.css", None, None),
    ("static_client.package_json", "apps/static-client/package.json", None, None),
    ("static_client.smoke_mjs", "apps/static-client/test/smoke.mjs", None, None),
    (
        "toolchain.e0_probe_source",
        "toolchain/probes/no_std_core/src/lib.rs",
        "10b2087683d3c2cb423768eb9c612c00ea929b171835c15d3d16792d6b8b19ac",
        "CODEX_HANDOFF.md section 6 (E0 probe source); "
        "toolchain/scripts/run_verus.sh SOURCE_SHA256_PIN",
    ),
    ("toolchain.e0_probe_manifest", "toolchain/probes/no_std_core/Cargo.toml", None, None),
    ("toolchain.e0_probe_lock", "toolchain/probes/no_std_core/Cargo.lock", None, None),
    ("toolchain.host_harness_lock", "toolchain/probes/host_harness/Cargo.lock", None, None),
    (
        "vertical_model.golden_basic_trace",
        "research/vertical-model/golden/basic.trace",
        "ab808dd308e3bdce0fa8cc2d3b9b4a14e87dbd1b41ae7143e897c53f7f3f1639",
        "CODEX_HANDOFF.md section 6 (vertical-model golden trace)",
    ),
    (
        "vertical_model.golden_coupled_trace",
        "research/vertical-model/golden/coupled.trace",
        None,
        "docs/implementation/VERTICAL_MODEL.md (second golden trace; not named "
        "in CODEX_HANDOFF.md section 6)",
    ),
    (
        "collateral_profiles.vectors",
        "research/collateral-profiles/vectors.json",
        "5bcf3a6117c4e411a5b9b339093eaf3dcd9ca1eee0bb7a2b6814a42f46639e48",
        "CODEX_HANDOFF.md section 6 (collateral-profile vectors)",
    ),
    (
        "collateral_profiles.identity_vectors",
        "research/collateral-profiles/identity_vectors.json",
        None,
        None,
    ),
    (
        "benchmarks.golden_checksums",
        "benchmarks/golden/checksums.sha256",
        None,
        "CODEX_HANDOFF.md section 6 points at this file rather than inlining "
        "the benchmark digests",
    ),
    ("benchmarks.golden_summary", "benchmarks/golden/SUMMARY.md", None, None),
    ("benchmarks.golden_matrix_csv", "benchmarks/golden/matrix.csv", None, None),
    ("benchmarks.golden_matrix_json", "benchmarks/golden/matrix.json", None, None),
    ("benchmarks.constants", "benchmarks/constants.json", None, None),
    ("fixtures.economics_admission", "fixtures/economics/admission_vectors.json", None, None),
    ("fixtures.economics_fee", "fixtures/economics/fee_vectors.json", None, None),
    ("fixtures.economics_trace", "fixtures/economics/trace_vectors.json", None, None),
    ("locks.clutch_kernel", "crates/clutch-kernel/Cargo.lock", None, None),
    ("locks.clutch_accumulator", "crates/clutch-accumulator/Cargo.lock", None, None),
    ("locks.clutch_batch", "crates/clutch-batch/Cargo.lock", None, None),
    ("locks.solana_layout", "programs/solana-layout/Cargo.lock", None, None),
    ("locks.solana_reference", "programs/solana-reference/Cargo.lock", None, None),
    ("locks.vertical_model", "research/vertical-model/Cargo.lock", None, None),
    ("toolchain.versions_env", "toolchain/versions.env", None, None),
    ("toolchain.pinned_proof_tools", "toolchain/PINNED_PROOF_TOOLS.md", None, None),
    ("proof_shadow.rocq_spec", "rocq/ClutchKernel.v", None, None),
    ("proof_shadow.verus_kernel", "verus/kernel/lib.rs", None, None),
    ("proof_shadow.verus_accumulator", "verus/accumulator/accumulator.rs", None, None),
    ("proof_shadow.verus_batch", "verus/batch/batch.rs", None, None),
]

# Digests that are not the sha256 of a file's bytes but of a declared
# canonicalization. Each carries its rule so an independent implementation can
# reproduce it without reading this script.
DERIVED_DIGESTS = [
    {
        "id": "static_client.canonical_terms",
        "source_path": "apps/static-client/terms.json",
        "rule": (
            "sha256 over UTF-8 of JSON.stringify of the `canonicalTerms` object "
            "with every object's keys sorted recursively and no whitespace "
            "(separators ',' and ':'), non-ASCII left unescaped"
        ),
        "handoff": "a21f6cbb1ab3b06afc7c8625f3388835843edb17c48173e8fb57df8b7e0dd8e8",
        "handoff_reference": (
            "CODEX_HANDOFF.md section 6 (static canonical terms); "
            "docs/implementation/STATIC_CLIENT.md; enforced by "
            "apps/static-client/test/smoke.mjs"
        ),
    }
]

# Identities the handoff declares that are *build outputs*, not repository
# bytes. They can only be confirmed by running the gate that produces them.
DECLARED_BUILD_OUTPUTS = [
    {
        "id": "toolchain.e0_sbf_rlib",
        "handoff": "d444c0ac118de1cb24d9fe6b509df7beafc1c0f1a8c2828b24e26b170da0ad1c",
        "handoff_reference": (
            "CODEX_HANDOFF.md section 6 (reproducible E0 SBF rlib); "
            "docs/implementation/TOOLCHAIN_SPIKE.md"
        ),
        "produced_by_gate": "toolchain.run_lab",
        "produced_by_output_key": "sbf_rlib_sha256",
        "note": (
            "an rlib emitted into a temporary directory by cargo-build-sbf, not "
            "a deployable program ELF and not a repository artifact"
        ),
    },
    {
        "id": "clutch_sbf.program_elf",
        "handoff": "59c48c482831626ae9d7cb908f4de0e3f93b1572cdd82105c61f2f87bdaad25f",
        "handoff_reference": (
            "docs/implementation/SBF_BRINGUP.md and CLAUDE_HANDOFF.md "
            "(same-machine reproducible deployable SBF ELF)"
        ),
        "produced_by_gate": "sbf.runtime_bringup",
        "produced_by_output_key": "sbf_elf_sha256",
        "note": (
            "deployable program ELF built twice into fresh target directories "
            "on one machine; byte identity is not independent reproducible-build "
            "closure and says nothing about deployment"
        ),
    },
]


# --------------------------------------------------------------------------
# Gate inventory. Commands are the verbatim CODEX_HANDOFF.md section 5 forms
# (loop expanded per manifest so each gate carries its own exit code), run
# through /bin/sh from the repository root.
# --------------------------------------------------------------------------

CARGO_MANIFESTS = [
    ("clutch_kernel", "crates/clutch-kernel/Cargo.toml", True),
    ("clutch_accumulator", "crates/clutch-accumulator/Cargo.toml", True),
    ("clutch_batch", "crates/clutch-batch/Cargo.toml", True),
    ("solana_layout", "programs/solana-layout/Cargo.toml", True),
    ("solana_reference", "programs/solana-reference/Cargo.toml", True),
    ("vertical_model", "research/vertical-model/Cargo.toml", False),  # no doc gate in section 5
    ("clutch_sbf", "programs/clutch-sbf/Cargo.toml", True),  # SBF lane (post-section-5; see note)
]

TEST_RESULT_PATTERNS = [r"^test result: "]
CLIPPY_PATTERNS = [r"^error(\[|:)", r"^warning(\[|:)"]
DOC_PATTERNS = [r"^\s*Documenting ", r"^error(\[|:)", r"^warning(\[|:)"]
UNITTEST_PATTERNS = [r"^Ran \d+ tests?$", r"^(OK|FAILED)\b", r"^(ERROR|FAIL): "]


def build_gates() -> list[dict[str, Any]]:
    gates: list[dict[str, Any]] = []

    for name, manifest, _has_doc in CARGO_MANIFESTS:
        gates.append(
            {
                "id": f"cargo_test.{name}",
                "section": "5",
                "command": f'cargo test --manifest-path "{manifest}" --offline --locked',
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": "unit and integration tests; not a proof and not runtime evidence",
            }
        )
    for name, manifest, _has_doc in CARGO_MANIFESTS:
        gates.append(
            {
                "id": f"cargo_clippy.{name}",
                "section": "5",
                "command": (
                    f'cargo clippy --manifest-path "{manifest}" --offline --locked '
                    "--all-targets -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "lint gate; no matched key lines is the clean state",
            }
        )
    for name, manifest, has_doc in CARGO_MANIFESTS:
        if not has_doc:
            continue
        gates.append(
            {
                "id": f"cargo_doc.{name}",
                "section": "5",
                "command": (
                    f"cargo doc --manifest-path {manifest} --offline --locked --no-deps"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": DOC_PATTERNS,
                "note": "documentation build; asserts nothing about content",
            }
        )

    gates.extend(
        [
            {
                "id": "vertical_model.golden_basic_trace",
                "section": "5",
                "command": (
                    "cargo run --quiet --manifest-path research/vertical-model/Cargo.toml "
                    "--offline --locked | cmp - research/vertical-model/golden/basic.trace"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r"^cmp: ", r" differ"],
                "note": "byte identity against the pinned scalar-lab trace",
            },
            {
                "id": "vertical_model.golden_coupled_trace",
                "section": "5-extended",
                "command": (
                    "cargo run --quiet --manifest-path research/vertical-model/Cargo.toml "
                    "--offline --locked -- coupled "
                    "| cmp - research/vertical-model/golden/coupled.trace"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r"^cmp: ", r" differ"],
                "note": (
                    "the coupled-relation trace is pinned by "
                    "docs/implementation/VERTICAL_MODEL.md but is not listed in "
                    "CODEX_HANDOFF.md section 5; added here so the second golden "
                    "artifact is covered"
                ),
            },
            {
                "id": "python.economics_unittest",
                "section": "5",
                "command": "python3 -m unittest discover -s research/economics -p 'test_*.py' -v",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": UNITTEST_PATTERNS,
                "note": "reference-model tests; MODEL evidence, not consensus code",
            },
            {
                "id": "python.collateral_profiles_unittest",
                "section": "5",
                "command": (
                    "python3 -m unittest discover -s research/collateral-profiles "
                    "-p 'test_*.py' -v"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": UNITTEST_PATTERNS,
                "note": "reference-model tests; MODEL evidence, not consensus code",
            },
            {
                "id": "python.collateral_profiles_lab",
                "section": "5",
                "command": "python3 research/collateral-profiles/run_lab.py",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [
                    r"^generic_profile_bytes=",
                    r"^generic_profile_digest=",
                    r"^dregg_dogfood_digest=",
                    r"^parent_profile_bytes=",
                    r"^generic_parent_identity=",
                    r"^dregg_parent_identity=",
                    r"^network_actions=",
                ],
                "note": "emits the collateral-profile identity digests it computes",
            },
            {
                "id": "benchmarks.cost_lab_check",
                "section": "5",
                "command": "python3 benchmarks/cost_lab.py check",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r"^check passed: "],
                "note": "cost model is a hypothesis (MODEL); no measured CU evidence",
            },
            {
                "id": "benchmarks.unittest",
                "section": "post-5",
                "command": "python3 -m unittest discover -s benchmarks/tests",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r"^Ran \d+ tests", r"^OK$", r"^FAILED"],
                "note": "benchmark harness tests incl. the landed-ABI arm",
            },
            {
                "id": "benchmarks.abi_audit",
                "section": "post-5",
                "command": "python3 benchmarks/cost_lab.py abi-audit",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r"drift", r"no drift", r"^abi-audit"],
                "note": (
                    "re-derives account widths from programs/solana-layout source; "
                    "refuses on ABI drift"
                ),
            },
            {
                "id": "benchmarks.golden_checksums",
                "section": "5",
                "command": "(cd benchmarks/golden && shasum -a 256 -c checksums.sha256)",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r": OK$", r": FAILED"],
                "note": "byte identity of the checked-in benchmark goldens",
            },
            {
                "id": "cargo_test.batch_policy_identity",
                "section": "post-5-research",
                "command": (
                    "cargo test --manifest-path research/batch-policy-identity/Cargo.toml "
                    "--locked --offline --all-targets"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": (
                    "bounded direct-selection authority model; host evidence only. "
                    "The measured V2 selection route remains a 1,400,000-CU STOP."
                ),
            },
            {
                "id": "cargo_clippy.batch_policy_identity",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --manifest-path research/batch-policy-identity/Cargo.toml "
                    "--locked --offline --all-targets -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the bounded direct-selection host model",
            },
            {
                "id": "cargo_test.bspline_shape_compiler",
                "section": "post-5-research",
                "command": (
                    "cargo test --manifest-path research/bspline-shape-compiler/Cargo.toml "
                    "--offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": (
                    "exact-rational host compiler and cross-language fixture checks; "
                    "not a consensus certificate or on-chain authority"
                ),
            },
            {
                "id": "cargo_clippy.bspline_shape_compiler",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --manifest-path research/bspline-shape-compiler/Cargo.toml "
                    "--all-targets --offline --locked -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the exact-rational host compiler",
            },
            {
                "id": "cargo_test.claim_algebra_model",
                "section": "post-5-research",
                "command": (
                    "cargo test --manifest-path research/claim-algebra-model/Cargo.toml "
                    "--offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": "bounded payoff-language MODEL evidence, not consensus code",
            },
            {
                "id": "cargo_clippy.claim_algebra_model",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --manifest-path research/claim-algebra-model/Cargo.toml "
                    "--offline --locked --all-targets -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the bounded payoff-language MODEL",
            },
            {
                "id": "cargo_doc.claim_algebra_model",
                "section": "post-5-research",
                "command": (
                    "RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path "
                    "research/claim-algebra-model/Cargo.toml --offline --locked --no-deps"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": DOC_PATTERNS,
                "note": "documentation build for the bounded payoff-language MODEL",
            },
            {
                "id": "cargo_test.claim_neutral_resolution",
                "section": "post-5-research",
                "command": (
                    "cargo test --manifest-path research/claim-neutral-resolution/Cargo.toml "
                    "--offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": "adversarial host model; not SBF implementation or runtime evidence",
            },
            {
                "id": "cargo_test_release.claim_neutral_resolution",
                "section": "post-5-research",
                "command": (
                    "cargo test --release --manifest-path "
                    "research/claim-neutral-resolution/Cargo.toml --offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": "release-profile run of the claim-neutral host model",
            },
            {
                "id": "cargo_clippy.claim_neutral_resolution",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --manifest-path research/claim-neutral-resolution/Cargo.toml "
                    "--offline --locked --all-targets --all-features -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the claim-neutral host model",
            },
            {
                "id": "cargo_doc.claim_neutral_resolution",
                "section": "post-5-research",
                "command": (
                    "cargo doc --manifest-path research/claim-neutral-resolution/Cargo.toml "
                    "--offline --locked --no-deps"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": DOC_PATTERNS,
                "note": "documentation build for the claim-neutral host model",
            },
            {
                "id": "cargo_test.fractional_redemption",
                "section": "post-5-research",
                "command": (
                    "cargo test --manifest-path research/fractional-redemption/Cargo.toml "
                    "--offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": "fractional-redemption MODEL evidence, not an SBF path",
            },
            {
                "id": "cargo_clippy.fractional_redemption",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --manifest-path research/fractional-redemption/Cargo.toml "
                    "--offline --locked --all-targets -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the fractional-redemption MODEL",
            },
            {
                "id": "cargo_test.liquidity_policy_model",
                "section": "post-5-research",
                "command": (
                    "cargo test --manifest-path research/liquidity-policy-model/Cargo.toml "
                    "--offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": "proof-constrained liquidity MODEL evidence, not live liquidity",
            },
            {
                "id": "cargo_clippy.liquidity_policy_model",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --manifest-path research/liquidity-policy-model/Cargo.toml "
                    "--offline --locked --all-targets -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the liquidity-policy MODEL",
            },
            {
                "id": "cargo_doc.liquidity_policy_model",
                "section": "post-5-research",
                "command": (
                    "RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path "
                    "research/liquidity-policy-model/Cargo.toml --offline --locked --no-deps"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": DOC_PATTERNS,
                "note": "documentation build for the liquidity-policy MODEL",
            },
            {
                "id": "python.liveness_policy_profile_unittest",
                "section": "post-5-research",
                "command": (
                    "python3 -m unittest discover -s research/liveness-policy-profile "
                    "-p 'test_*.py'"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": UNITTEST_PATTERNS,
                "note": (
                    "historical liveness-profile arithmetic tests; the sealed ELF "
                    "replay artifact is not committed and this is not final-ELF evidence"
                ),
            },
            {
                "id": "cargo_test.liveness_policy_profile",
                "section": "post-5-research",
                "command": (
                    "cargo test --offline --locked --manifest-path "
                    "research/liveness-policy-profile/Cargo.toml"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": "local account-size and rent evidence probe, not runtime integration",
            },
            {
                "id": "cargo_clippy.liveness_policy_profile",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --offline --locked --manifest-path "
                    "research/liveness-policy-profile/Cargo.toml --all-targets -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the local liveness account probe",
            },
            {
                "id": "lp_mapping_probe.release_run",
                "section": "post-5-research",
                "command": (
                    "(cd research/lp-mapping-probe && cargo run --release --offline --locked)"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r"^== E[1-4]  "],
                "note": "PROPOSED host-side falsifier probe; not a shipped or on-chain crate",
            },
            {
                "id": "cargo_test.resolution_work_v1",
                "section": "post-5-research",
                "command": (
                    "cargo test --manifest-path research/resolution-work-v1/Cargo.toml "
                    "--offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": (
                    "isolated resumable-work model; the separately gated local SBF "
                    "route is not an end-to-end deployment claim"
                ),
            },
            {
                "id": "cargo_test_release.resolution_work_v1",
                "section": "post-5-research",
                "command": (
                    "cargo test --release --manifest-path research/resolution-work-v1/Cargo.toml "
                    "--offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": "release-profile run of the isolated resumable-work model",
            },
            {
                "id": "cargo_clippy.resolution_work_v1",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --manifest-path research/resolution-work-v1/Cargo.toml "
                    "--offline --locked --all-targets -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the isolated resumable-work model",
            },
            {
                "id": "cargo_doc.resolution_work_v1",
                "section": "post-5-research",
                "command": (
                    "cargo doc --manifest-path research/resolution-work-v1/Cargo.toml "
                    "--offline --locked --no-deps"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": DOC_PATTERNS,
                "note": "documentation build for the isolated resumable-work model",
            },
            {
                "id": "cargo_test.source_profile_v1",
                "section": "post-5-research",
                "command": (
                    "cargo test --manifest-path research/source-profile-v1/Cargo.toml "
                    "--offline --locked"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": TEST_RESULT_PATTERNS,
                "note": (
                    "conditional source parser/model; it does not make Endow's "
                    "registered production-source gate pass"
                ),
            },
            {
                "id": "cargo_clippy.source_profile_v1",
                "section": "post-5-research",
                "command": (
                    "cargo clippy --manifest-path research/source-profile-v1/Cargo.toml "
                    "--offline --locked --all-targets -- -D warnings"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": CLIPPY_PATTERNS,
                "note": "strict lint for the conditional source-profile model",
            },
            {
                "id": "cargo_doc.source_profile_v1",
                "section": "post-5-research",
                "command": (
                    "RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path "
                    "research/source-profile-v1/Cargo.toml --offline --locked --no-deps"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": DOC_PATTERNS,
                "note": "documentation build for the conditional source-profile model",
            },
            {
                "id": "python.economics_lab",
                "section": "post-5-research",
                "command": "python3 research/economics/run_lab.py",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r'^  "status": '],
                "note": "stable scenario report from the economics MODEL, not a fee policy",
            },
            {
                "id": "python.economics_admission_unittest",
                "section": "post-5-research",
                "command": (
                    "python3 -m unittest discover -s research/economics-admission "
                    "-p 'test_*.py'"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": UNITTEST_PATTERNS,
                "note": "admission and fee-policy falsifier; MODEL evidence only",
            },
            {
                "id": "python.economics_admission_lab",
                "section": "post-5-research",
                "command": "python3 research/economics-admission/run_lab.py",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r'^  "status": '],
                "note": "stable admission/fee-policy scenario report; no policy promotion",
            },
            {
                "id": "python.structured_claim_wrapper_unittest",
                "section": "post-5-research",
                "command": (
                    "python3 -m unittest discover -s research/structured-claim-wrapper "
                    "-p 'test_*.py' -v"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": UNITTEST_PATTERNS,
                "note": "structured-claim wrapper research model; not a Token-2022 path",
            },
            {
                "id": "python.structured_claim_wrapper_lab",
                "section": "post-5-research",
                "command": "python3 research/structured-claim-wrapper/run_lab.py",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r"^coefficients: ", r"^universal exact redemption lot: "],
                "note": "stable structured-claim wrapper scenario report; MODEL evidence only",
            },
            {
                "id": "python.bspline_window_semantics_unittest",
                "section": "post-5-research",
                "command": (
                    "python3 -m unittest discover -s research/bspline-window-semantics "
                    "-p 'test_*.py' -v"
                ),
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": UNITTEST_PATTERNS,
                "note": "isolated exact window-semantics model, not a settlement route",
            },
            {
                "id": "python.bspline_window_semantics_compare",
                "section": "post-5-research",
                "command": "python3 research/bspline-window-semantics/compare.py",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [r"^basis: ", r"^path: ", r"^exact-basis occupation: "],
                "note": "deterministic comparison for the isolated window-semantics model",
            },
            {
                "id": "sbf.runtime_bringup",
                "section": "post-5-runtime",
                "command": "programs/clutch-sbf/scripts/run_bringup.sh",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [
                    r"^pass [12]  sha256=[0-9a-f]{64}  bytes=[0-9]+$",
                    r"^sbf_reproducibility=PASS$",
                    r"^final_elf_stack_diagnostic_symbols=ABSENT ",
                    r"^validator executed program readiness probe ",
                    r"^[0-9]+ accepting transactions$",
                    r"^  accept ",
                    r"^  refuse ",
                    r"^\s+[0-9]+\s+walk-",
                    r"^\s+terminal identity:",
                    r"^one byte .* went red:$",
                    r"^the terminal .* went red:$",
                    r"^one payout .* went red:$",
                    r"^PASS$",
                    r"^sbf_elf_sha256=[0-9a-f]{64}$",
                ],
                "note": (
                    "builds the deployable ELF twice, confirms byte identity on "
                    "one machine, rejects any backend stack-diagnostic symbol "
                    "that survives final-ELF LTO, then runs the entrypoint, "
                    "per-family differential/refusal matrix, falsifiability "
                    "checks, and ordered lifecycle on a loopback validator; "
                    "dependency diagnostics proven absent from the linked ELF "
                    "are build diagnostics, not evidence of reachable undefined "
                    "behavior, but their absence is not a general stack-safety proof"
                ),
            },
            {
                "id": "sbf.token2022_program_test",
                "section": "post-5-runtime",
                "command": "programs/clutch-sbf/svm-tests/run_svm_tests.sh",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [
                    r"^[0-9a-f]{64}\s+.*clutch_sbf\.so$",
                    r"^running [0-9]+ tests?$",
                    r"^test [a-zA-Z0-9_]+ .*",
                    r"^SVM ",
                    r"^test result: ",
                ],
                "note": (
                    "executes the real SBF ELF and Token-2022 program in an "
                    "in-process Agave bank, including extension refusals, "
                    "mandatory token/collateral planes, and E5 atomic rollback; "
                    "program-test is not a cluster, deployment, or runtime-"
                    "diversity result"
                ),
            },
            {
                "id": "static_client.npm",
                "section": "5",
                "command": "(cd apps/static-client && npm test && npm run check)",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [
                    # node:test runner summary (per-test lines carry timings and are
                    # deliberately not captured), plus the legacy single-line form.
                    r"^ℹ (tests|suites|pass|fail|cancelled|skipped|todo) \d+$",
                    r"^✖ ",
                    r"^not ok ",
                    r"^static-client smoke: ",
                    r"^AssertionError",
                ],
                "note": (
                    "32 offline Node checks cover the embedded evidence lens, strict "
                    "snapshot validation, local-asset/CSP boundaries, accessibility, "
                    "and canonical terms digest; no package runtime dependencies"
                ),
            },
            {
                "id": "toolchain.run_lab",
                "section": "5",
                "command": "CARGO_NET_OFFLINE=true toolchain/scripts/run_lab.sh",
                "expected": {"mode": "zero", "exit": 0},
                "key_patterns": [
                    r"^lab_schema=",
                    r"^source_sha256=",
                    r"^host_toolchain=",
                    r"^host_rustc=",
                    r"^sbf_build=",
                    r"^host_build=",
                    r"^sbf_rlib_sha256",
                    r"^sbf_reproducibility=",
                    r"^prohibited_source_scan=",
                    r"^verus(_probe)?=",
                    r"^compatibility=",
                ],
                "volatile_patterns": [r"^host_rlib_sha256="],
                "volatile_reason": (
                    "host_rlib_sha256 is not stable across runs: run_lab.sh builds "
                    "the host probe into a fresh mktemp target directory and the "
                    "path is embedded in the artifact. run_lab.sh never rebuilds "
                    "the host side, so it measures no host reproducibility and "
                    "TOOLCHAIN_SPIKE.md claims none. The value is recorded as "
                    "evidence but excluded from the drift digest. Only "
                    "sbf_rlib_sha256 is measured reproducible, and only by one "
                    "same-machine rebuild."
                ),
                "note": (
                    "E0 host+SBF compatibility probe; the SBF product is an rlib, "
                    "not a deployable ELF, and reproducibility here is a single "
                    "same-machine rebuild comparison"
                ),
            },
            {
                "id": "proof.verus_probe",
                "section": "5-expected-unavailable",
                "command": "toolchain/scripts/run_verus.sh",
                "expected": {
                    "mode": "nonzero",
                    "exit": None,
                    "reason": (
                        "the pinned Verus release rejects the pinned probe source "
                        "(verus_builtin crate not imported). Making it pass requires "
                        "editing the probe, which changes the pinned source digest, so "
                        "the correct state is a recorded failure, not a green gate."
                    ),
                },
                "proof_content": "none",
                "key_patterns": [
                    r"^verus_observed_version=",
                    r"^verus_observed_toolchain=",
                    r"^verus_pinned_version=",
                    r"^source_sha256=",
                    r"^source_sha256_pin=",
                    r"^BLOCKED: ",
                    r"^error: ",
                ],
                "note": "BLOCKER (CODEX_HANDOFF.md section 7 P0-2): formal-tool gap",
            },
            {
                "id": "proof.rocq_check",
                "section": "5-expected-unavailable",
                "command": "rocq/check.sh",
                "expected": {
                    "mode": "either",
                    "accepted_exits": [0, 2],
                    "reason": (
                        "exit 0 means rocq/ClutchKernel.v elaborates; exit 2 means no "
                        "rocq/coqc is on PATH. Neither outcome carries proof content: "
                        "the file contains zero theorems, only `Definition ... : Prop` "
                        "obligations, one of which "
                        "(successful_transition_is_well_formed) has a machine-checked "
                        "vacuous conjunct."
                    ),
                },
                "proof_content": "none",
                "key_patterns": [r"^status=", r"^rocq=", r"^coqc=", r"^reason="],
                "note": "BLOCKER (CODEX_HANDOFF.md section 7 P0-2): typecheck is not proof",
            },
        ]
    )
    return gates


# --------------------------------------------------------------------------
# Helpers
# --------------------------------------------------------------------------


def die(message: str, code: int = EXIT_ENVIRONMENT) -> None:
    print(f"baseline_manifest: {message}", file=sys.stderr)
    raise SystemExit(code)


def run_capture(args: list[str], cwd: Path) -> tuple[int, str]:
    proc = subprocess.run(
        args, cwd=str(cwd), capture_output=True, text=True, check=False
    )
    return proc.returncode, proc.stdout + proc.stderr


def run_capture_bytes(args: list[str], cwd: Path) -> tuple[int, bytes]:
    proc = subprocess.run(args, cwd=str(cwd), capture_output=True, check=False)
    return proc.returncode, proc.stdout


def git(repo: Path, *args: str) -> str:
    code, out = run_capture(["git", *args], repo)
    if code != 0:
        die(f"git {' '.join(args)} failed: {out.strip()}")
    return out


def repo_root(start: Path) -> Path:
    code, out = run_capture(["git", "rev-parse", "--show-toplevel"], start)
    if code != 0:
        die("not inside a git repository; a baseline manifest needs a git identity")
    return Path(out.strip())


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    """Sorted-key, compact, UTF-8 JSON. Matches apps/static-client canonicalization."""
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    ).encode("utf-8")


TIMING_SUBS = [
    (re.compile(r"; finished in [0-9.]+s"), ""),
    (re.compile(r"^(Ran \d+ tests?) in [0-9.]+s$"), r"\1"),
    (re.compile(r"^(OK|FAILED)(.*?)\s*$"), r"\1\2"),
    (re.compile(r"\bin [0-9]+\.[0-9]+s\b"), "in <elapsed>"),
]


def normalize_line(line: str) -> str:
    out = line.rstrip()
    for pattern, replacement in TIMING_SUBS:
        out = pattern.sub(replacement, out)
    return out


def extract_key_lines(output: str, patterns: list[str], cap: int = 256) -> list[str]:
    compiled = [re.compile(p) for p in patterns]
    seen: list[str] = []
    for raw in output.splitlines():
        line = normalize_line(raw)
        if not line:
            continue
        if any(p.search(line) for p in compiled):
            if line not in seen:
                seen.append(line)
            if len(seen) >= cap:
                break
    return seen


def gate_outcome_ok(expected: dict[str, Any], exit_code: int) -> bool:
    mode = expected["mode"]
    if mode == "zero":
        return exit_code == 0
    if mode == "nonzero":
        return exit_code != 0
    if mode == "either":
        return exit_code in expected.get("accepted_exits", [])
    raise ValueError(f"unknown expectation mode {mode!r}")


# --------------------------------------------------------------------------
# Derivation
# --------------------------------------------------------------------------


def collect_worktree_state(repo: Path, exclude: list[str]) -> dict[str, Any]:
    """NUL-safe porcelain status, minus only the manifest output itself.

    Rename/copy records name two paths.  Such a record is excluded only when
    *both* paths name the manifest; a rename between the manifest and any other
    path remains dirty.  This prevents the output exemption from swallowing a
    source deletion or insertion hidden in a rename record.
    """
    code, raw = run_capture_bytes(
        ["git", "status", "--porcelain=v1", "-z", "--untracked-files=all"], repo
    )
    if code != 0:
        die("git status --porcelain=v1 -z failed")
    excluded_bytes = {os.fsencode(path) for path in exclude}
    chunks = raw.split(b"\0")
    entries: list[str] = []
    excluded: list[str] = []
    index = 0
    while index < len(chunks):
        item = chunks[index]
        index += 1
        if not item:
            continue
        if len(item) < 4 or item[2:3] != b" ":
            die("could not parse git status --porcelain=v1 -z output")
        status_code = item[:2]
        paths = [item[3:]]
        if b"R" in status_code or b"C" in status_code:
            if index >= len(chunks) or not chunks[index]:
                die("truncated rename/copy record in git status output")
            paths.append(chunks[index])
            index += 1
        rendered = f"{status_code.decode('ascii', 'replace')} " + " -> ".join(
            os.fsdecode(path) for path in paths
        )
        target = excluded if all(path in excluded_bytes for path in paths) else entries
        target.append(rendered)
    return {
        "dirty": bool(entries),
        "porcelain": entries,
        "excluded": excluded,
        "exclude_paths": exclude,
    }


def _identity_record(
    hasher: "hashlib._Hash",
    *,
    path: bytes,
    mode: str,
    kind: str,
    size: int,
    digest: bytes,
) -> None:
    """Append one unambiguous length-delimited record to a content identity."""
    fields = (mode.encode("ascii"), kind.encode("ascii"), path, digest)
    for field in fields:
        hasher.update(len(field).to_bytes(8, "big"))
        hasher.update(field)
    hasher.update(size.to_bytes(8, "big", signed=False))


def collect_content_identity(repo: Path, exclude: list[str]) -> dict[str, Any]:
    """Hash every tracked working-tree byte except this generated artifact.

    This is deliberately independent of commit metadata.  A checked-in
    manifest necessarily changes HEAD and HEAD^{tree}; excluding the artifact
    from a byte identity makes the identity stable across that one manifest-only
    commit while still binding every other tracked path, its Git mode/type, and
    its working-tree bytes.
    """
    code, raw = run_capture_bytes(["git", "ls-files", "--stage", "-z"], repo)
    if code != 0:
        die("git ls-files --stage -z failed")
    excluded_bytes = {os.fsencode(path) for path in exclude}
    hasher = hashlib.sha256()
    hasher.update(CONTENT_IDENTITY_SCHEMA.encode("ascii") + b"\0")
    count = 0
    excluded_observed: list[str] = []

    rows: list[tuple[bytes, str, str]] = []
    for item in raw.split(b"\0"):
        if not item:
            continue
        metadata, separator, path = item.partition(b"\t")
        if not separator:
            die("could not parse git ls-files --stage -z output")
        pieces = metadata.split(b" ")
        if len(pieces) != 3:
            die("could not parse an index entry while deriving content identity")
        mode_raw, object_id_raw, stage_raw = pieces
        if stage_raw != b"0":
            die(
                "unmerged index entry prevents a canonical content identity: "
                + os.fsdecode(path)
            )
        rows.append((path, mode_raw.decode("ascii"), object_id_raw.decode("ascii")))

    for path_raw, index_mode, object_id in sorted(rows, key=lambda row: row[0]):
        if path_raw in excluded_bytes:
            excluded_observed.append(os.fsdecode(path_raw))
            continue
        path = repo / os.fsdecode(path_raw)
        try:
            info = path.lstat()
        except FileNotFoundError:
            info = None
        if info is None:
            # Strict emission refuses this state before identity collection.  A
            # dirty snapshot still gets a deterministic sentinel rather than a
            # misleading hash of the index copy.
            mode = "missing"
            kind = "missing"
            payload = b""
        elif index_mode == "160000":
            # A gitlink's repository content is the pinned object id.  Dirt in
            # a checked-out submodule is separately visible to git status.
            mode = "160000"
            kind = "gitlink"
            payload = object_id.encode("ascii")
        elif stat.S_ISLNK(info.st_mode):
            mode = "120000"
            kind = "symlink-target"
            payload = os.fsencode(os.readlink(path))
        elif stat.S_ISREG(info.st_mode):
            mode = "100755" if info.st_mode & 0o111 else "100644"
            kind = "regular"
            payload = path.read_bytes()
        else:
            mode = f"special-{stat.S_IFMT(info.st_mode):o}"
            kind = "unsupported-special"
            payload = b""
        digest = hashlib.sha256(payload).digest()
        _identity_record(
            hasher,
            path=path_raw,
            mode=mode,
            kind=kind,
            size=len(payload),
            digest=digest,
        )
        count += 1

    return {
        "schema": CONTENT_IDENTITY_SCHEMA,
        "algorithm": "sha256",
        "sha256": hasher.hexdigest(),
        "entry_count": count,
        "excluded_paths": exclude,
        "excluded_tracked_paths_observed": excluded_observed,
        "rule": (
            "enumerate stage-0 entries from `git ls-files --stage -z`, sort by "
            "raw path bytes, exclude only excluded_paths, and hash the schema "
            "tag followed by length-delimited (observed Git mode/type, kind, "
            "raw path, sha256(payload), payload length) records. Payload is "
            "working-tree bytes for regular files, link-target bytes for "
            "symlinks, and the index object id for gitlinks. Strict cleanliness "
            "makes this a complete identity of all checked-in gate/source bytes "
            "other than the generated manifest itself."
        ),
    }


def collect_provenance(repo: Path) -> dict[str, Any]:
    commit = git(repo, "rev-parse", "HEAD").strip()
    tree = git(repo, "rev-parse", "HEAD^{tree}").strip()
    subject = git(repo, "log", "-1", "--format=%s").strip()
    code, remotes = run_capture(["git", "remote", "-v"], repo)
    remote_list = []
    if code == 0:
        for line in remotes.splitlines():
            parts = line.split()
            if len(parts) >= 3 and parts[2] == "(fetch)":
                remote_list.append({"name": parts[0], "fetch_url": parts[1]})
    code, tags = run_capture(["git", "tag", "--points-at", "HEAD"], repo)
    tag_list = [t for t in tags.splitlines() if t.strip()] if code == 0 else []
    return {
        "emitted_from_commit": commit,
        "emitted_from_tree_hash": tree,
        "commit_subject": subject,
        "remotes": remote_list,
        "tags_at_head": tag_list,
        "note": (
            "historical emission context only; these fields are not the "
            "checkable content identity because committing this generated "
            "manifest necessarily changes HEAD and HEAD^{tree}. Git object ids "
            "and configured remote names are not provenance attestations. A configured "
            "remote is not provenance and a pushed branch is not a release: "
            "neither is signed, neither is tagged, and neither is attested "
            "here. An empty `tags_at_head` means no release tag exists, which "
            "is the expected state while CODEX_HANDOFF.md section 7 P0-1 is open."
        ),
    }


def collect_digests(repo: Path) -> dict[str, Any]:
    entries: dict[str, Any] = {}
    missing: list[str] = []

    for entry_id, rel, handoff, reference in FILE_DIGESTS:
        path = repo / rel
        record: dict[str, Any] = {"path": rel, "kind": "file-sha256"}
        if not path.is_file():
            record["sha256"] = None
            record["status"] = "MISSING"
            missing.append(rel)
        else:
            record["sha256"] = sha256_file(path)
            record["bytes"] = path.stat().st_size
            record["status"] = "present"
        if handoff is not None:
            record["handoff_declared_sha256"] = handoff
            record["matches_handoff"] = record["sha256"] == handoff
        if reference is not None:
            record["handoff_reference"] = reference
        entries[entry_id] = record

    for spec in DERIVED_DIGESTS:
        path = repo / spec["source_path"]
        record = {
            "path": spec["source_path"],
            "kind": "derived-sha256",
            "rule": spec["rule"],
        }
        if not path.is_file():
            record["sha256"] = None
            record["status"] = "MISSING"
            missing.append(spec["source_path"])
        else:
            terms = json.loads(path.read_text(encoding="utf-8"))
            payload = terms.get("canonicalTerms")
            if payload is None:
                record["sha256"] = None
                record["status"] = "MISSING_FIELD:canonicalTerms"
            else:
                record["sha256"] = hashlib.sha256(
                    canonical_json_bytes(payload)
                ).hexdigest()
                record["status"] = "present"
        record["handoff_declared_sha256"] = spec["handoff"]
        record["matches_handoff"] = record["sha256"] == spec["handoff"]
        record["handoff_reference"] = spec["handoff_reference"]
        entries[spec["id"]] = record

    for spec in DECLARED_BUILD_OUTPUTS:
        entries[spec["id"]] = {
            "kind": "declared-build-output",
            "sha256": None,
            "handoff_declared_sha256": spec["handoff"],
            "handoff_reference": spec["handoff_reference"],
            "produced_by_gate": spec["produced_by_gate"],
            "produced_by_output_key": spec["produced_by_output_key"],
            "note": spec["note"],
            "status": "not-a-repository-file",
        }

    return {"entries": entries, "missing_paths": missing}


VERSIONS_ENV_LINE = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=(.*)$")


def parse_versions_env(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        match = VERSIONS_ENV_LINE.match(line)
        if not match:
            continue
        key, value = match.group(1), match.group(2).strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
            value = value[1:-1]
        values[key] = value
    return values


# sha256 values PINNED_PROOF_TOOLS.md states in prose/tables, keyed by the
# versions.env variable they must agree with.
PINNED_DOC_CROSSCHECKS = [
    ("VERUS_ARTIFACT_SHA256", "verus artifact zip"),
    ("VERUS_BIN_SHA256", "verus binary"),
    ("VERUS_Z3_SHA256", "verus bundled z3"),
    ("VERUS_VSTD_RLIB_SHA256", "libvstd.rlib"),
    ("VERUS_VSTD_VIR_SHA256", "vstd.vir"),
    ("ROCQ_SOURCE_SHA256", "rocq upstream tarball"),
    ("ROCQ_BOTTLE_SHA256", "rocq homebrew bottle"),
]


def collect_toolchain(repo: Path) -> dict[str, Any]:
    versions_path = repo / "toolchain/versions.env"
    pinned_path = repo / "toolchain/PINNED_PROOF_TOOLS.md"
    if not versions_path.is_file():
        die("toolchain/versions.env is missing; refusing to emit a toolchain block")
    if not pinned_path.is_file():
        die("toolchain/PINNED_PROOF_TOOLS.md is missing; refusing to emit a toolchain block")

    env = parse_versions_env(versions_path)
    pinned_text = pinned_path.read_text(encoding="utf-8")
    pinned_hashes = set(re.findall(r"\b[0-9a-f]{64}\b", pinned_text))

    crosscheck = {}
    for key, label in PINNED_DOC_CROSSCHECKS:
        value = env.get(key)
        crosscheck[key] = {
            "label": label,
            "versions_env": value,
            "present_in_pinned_proof_tools_md": bool(value) and value in pinned_hashes,
        }

    return {
        "records": {
            "versions_env": {
                "path": "toolchain/versions.env",
                "sha256": sha256_file(versions_path),
            },
            "pinned_proof_tools_md": {
                "path": "toolchain/PINNED_PROOF_TOOLS.md",
                "sha256": sha256_file(pinned_path),
            },
        },
        "host": {
            "rust_toolchain": env.get("HOST_RUST_TOOLCHAIN"),
            "rust_version": env.get("HOST_RUST_VERSION"),
            "sbf_cli_version": env.get("SBF_CLI_VERSION"),
            "sbf_build_version": env.get("SBF_BUILD_VERSION"),
            "sbf_platform_tools_version": env.get("SBF_PLATFORM_TOOLS_VERSION"),
            "sbf_rust_version": env.get("SBF_RUST_VERSION"),
            "z3_version": env.get("Z3_VERSION"),
        },
        "verus": {
            "version": env.get("VERUS_VERSION"),
            "release_tag": env.get("VERUS_RELEASE_TAG"),
            "commit": env.get("VERUS_COMMIT"),
            "artifact": env.get("VERUS_ARTIFACT"),
            "artifact_sha256": env.get("VERUS_ARTIFACT_SHA256"),
            "binary_sha256": env.get("VERUS_BIN_SHA256"),
            "rust_frontend_toolchain": env.get("VERUS_RUST_TOOLCHAIN"),
            "bundled_z3": env.get("VERUS_Z3_BUNDLED"),
            "bundled_z3_sha256": env.get("VERUS_Z3_SHA256"),
            "vstd_rlib_sha256": env.get("VERUS_VSTD_RLIB_SHA256"),
            "vstd_vir_sha256": env.get("VERUS_VSTD_VIR_SHA256"),
            "probe_status": env.get("VERUS_PROBE_STATUS"),
            "probe_reason": env.get("VERUS_PROBE_REASON"),
            "note": (
                "the Verus Rust frontend pin is independent of the host build "
                "toolchain and must not be conflated with it; vstd has no "
                "independent revision and is pinned only transitively by the "
                "Verus commit"
            ),
        },
        "rocq": {
            "version": env.get("ROCQ_VERSION"),
            "release": env.get("ROCQ_RELEASE"),
            "install_method": env.get("ROCQ_INSTALL_METHOD"),
            "source_url": env.get("ROCQ_SOURCE_URL"),
            "source_sha256": env.get("ROCQ_SOURCE_SHA256"),
            "bottle_sha256": env.get("ROCQ_BOTTLE_SHA256"),
            "ocaml_version": env.get("ROCQ_OCAML_VERSION"),
            "check_status": env.get("ROCQ_CHECK_STATUS"),
            "check_proof_content": env.get("ROCQ_CHECK_PROOF_CONTENT"),
        },
        "pin_agreement": {
            "rule": (
                "each sha256 recorded in versions.env must also appear literally "
                "in PINNED_PROOF_TOOLS.md"
            ),
            "checks": crosscheck,
            "all_agree": all(
                item["present_in_pinned_proof_tools_md"] for item in crosscheck.values()
            ),
        },
        "unpinned": [
            "vstd revision (transitive via VERUS_COMMIT only)",
            "homebrew formula provenance (JSON API install, no tap commit)",
            "rocq stdlib / dependency closure (no lockfile)",
            "librustc_driver dylib supplied by the ambient rustup toolchain",
            "correspondence between the Verus/Rocq shadows and crates/clutch-*",
        ],
    }


def run_gates(repo: Path, gates: list[dict[str, Any]], timeout: int) -> dict[str, Any]:
    results: dict[str, Any] = {}
    env = dict(os.environ)
    env.setdefault("CARGO_NET_OFFLINE", "true")
    env["CARGO_TERM_COLOR"] = "never"
    env["RUSTUP_SKIP_UPDATE_CHECK"] = "1"
    env["NO_COLOR"] = "1"
    env["LC_ALL"] = "C"

    for gate in gates:
        gate_id = gate["id"]
        print(f"  gate {gate_id} ...", file=sys.stderr, flush=True)
        try:
            proc = subprocess.run(
                ["/bin/sh", "-c", gate["command"]],
                cwd=str(repo),
                capture_output=True,
                text=True,
                check=False,
                timeout=timeout,
                env=env,
            )
            exit_code = proc.returncode
            output = proc.stdout + proc.stderr
            timed_out = False
        except subprocess.TimeoutExpired as exc:
            exit_code = -1
            output = (exc.stdout or "") + (exc.stderr or "")
            if isinstance(output, bytes):  # pragma: no cover - platform dependent
                output = output.decode("utf-8", "replace")
            timed_out = True

        key_lines = extract_key_lines(output, gate["key_patterns"])
        volatile = extract_key_lines(output, gate.get("volatile_patterns", []))
        ok = (not timed_out) and gate_outcome_ok(gate["expected"], exit_code)
        results[gate_id] = {
            "exit_code": exit_code,
            "timed_out": timed_out,
            "matches_expectation": ok,
            "key_lines": key_lines,
            # Digest over key_lines only. volatile_lines are recorded as evidence
            # and never enter the drift comparison.
            "key_lines_sha256": hashlib.sha256(
                "\n".join(key_lines).encode("utf-8")
            ).hexdigest(),
            **({"volatile_lines": volatile} if volatile else {}),
            "output_bytes": len(output.encode("utf-8")),
            "tail": [normalize_line(x) for x in output.splitlines()[-12:]] if not ok else [],
        }
        print(
            f"    exit={exit_code} expected={gate['expected']['mode']} "
            f"{'OK' if ok else 'MISMATCH'}",
            file=sys.stderr,
            flush=True,
        )
    return results


def summarize_unavailable(
    gates: list[dict[str, Any]], results: dict[str, Any] | None
) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for gate in gates:
        expected = gate["expected"]
        declared = expected["mode"] != "zero"
        result = (results or {}).get(gate["id"])
        unexpected = result is not None and not result["matches_expectation"]
        if not declared and not unexpected:
            continue
        entry = {
            "gate": gate["id"],
            "command": gate["command"],
            "disposition": (
                "declared-failing"
                if expected["mode"] == "nonzero"
                else "declared-typecheck-or-unavailable"
                if expected["mode"] == "either"
                else "unexpected-failure"
            ),
            "reason": expected.get("reason")
            or "gate outcome contradicted its declaration; see gate_runs",
        }
        if "proof_content" in gate:
            entry["proof_content"] = gate["proof_content"]
        if result is not None:
            entry["observed_exit_code"] = result["exit_code"]
            entry["matches_expectation"] = result["matches_expectation"]
        out.append(entry)
    return out


def gate_manifest_record(gate: dict[str, Any]) -> dict[str, Any]:
    """The complete stable declaration stored for one executable gate."""
    return {
        "id": gate["id"],
        "section": gate["section"],
        "command": gate["command"],
        "cwd": ".",
        "shell": "/bin/sh -c",
        "expected": gate["expected"],
        "key_patterns": gate["key_patterns"],
        "note": gate["note"],
        **(
            {
                "volatile_patterns": gate["volatile_patterns"],
                "volatile_reason": gate["volatile_reason"],
            }
            if "volatile_patterns" in gate
            else {}
        ),
        **({"proof_content": gate["proof_content"]} if "proof_content" in gate else {}),
    }


def build_manifest(
    repo: Path,
    *,
    allow_dirty: bool,
    with_gates: bool,
    gate_timeout: int,
    script_path: Path,
    out_rel: str,
) -> tuple[dict[str, Any], int]:
    worktree = collect_worktree_state(repo, [out_rel])
    if worktree["dirty"] and not allow_dirty:
        print(
            "baseline_manifest: REFUSING to emit a baseline manifest: the working "
            "tree is dirty.\n"
            "A manifest emitted now would claim a clean baseline it does not have.\n"
            "Commit or stash the following, or pass --allow-dirty for a labelled "
            "mid-flight snapshot:",
            file=sys.stderr,
        )
        for line in worktree["porcelain"]:
            print(f"  {line}", file=sys.stderr)
        raise SystemExit(EXIT_DIRTY_REFUSAL)

    gates = build_gates()
    digests = collect_digests(repo)
    toolchain = collect_toolchain(repo)

    manifest: dict[str, Any] = {
        "schema": SCHEMA,
        "generator": {
            "path": "scripts/baseline_manifest.py",
            "sha256": sha256_file(script_path),
            "stdlib_only": True,
        },
        "dirty": worktree["dirty"],
        "dirty_check_excludes": {
            "paths": worktree["exclude_paths"],
            "observed": worktree["excluded"],
            "reason": (
                "the manifest is this tool's own output; its git status cannot "
                "gate its regeneration. Nothing else is excluded from the "
                "dirtiness decision."
            ),
        },
    }
    if worktree["dirty"]:
        manifest["dirty_warning"] = (
            "THIS IS NOT A BASELINE. The working tree was dirty when this manifest "
            "was emitted. content_identity describes tracked working-tree bytes, "
            "but untracked paths are not included and the provenance commit/tree do "
            "not describe the tree the gates ran against. Mid-flight snapshot only; "
            "regenerate on a clean tree before citing it as evidence."
        )
        manifest["dirty_porcelain"] = worktree["porcelain"]

    manifest["baseline"] = {
        "content_identity": collect_content_identity(repo, [out_rel]),
        "provenance": collect_provenance(repo),
        "self_reference_policy": (
            "the generated manifest is excluded from content_identity and from "
            "the strict dirty decision. Its bytes cannot truthfully attest "
            "themselves. Every other tracked path is bound, and every other "
            "dirty path makes strict emission refuse."
        ),
    }
    manifest["claims"] = {
        "verified": False,
        "deployed": False,
        "release": False,
        "reviewed_offline_checks_recorded": with_gates,
        "label_vocabulary": LABEL_VOCABULARY,
        "manifest_label": "IMPLEMENTED" if not worktree["dirty"] else "PROPOSED",
        "not_attested": NOT_ATTESTED,
    }
    manifest["gates"] = [gate_manifest_record(gate) for gate in gates]
    manifest["digests"] = digests["entries"]
    if digests["missing_paths"]:
        manifest["missing_paths"] = digests["missing_paths"]
    manifest["toolchain"] = toolchain

    results = None
    exit_code = EXIT_OK
    if with_gates:
        started = time.time()
        print("baseline_manifest: running gates", file=sys.stderr, flush=True)
        results = run_gates(repo, gates, gate_timeout)
        manifest["run"] = {
            "started_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(started)),
            "finished_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "gate_timeout_seconds": gate_timeout,
            "note": (
                "timestamps here are the only nondeterministic fields in this "
                "manifest; `check` ignores them"
            ),
        }
        manifest["gate_runs"] = results
        manifest["gate_summary"] = {
            "total": len(gates),
            "matching_expectation": sum(
                1 for r in results.values() if r["matches_expectation"]
            ),
            "contradicting_expectation": sorted(
                gid for gid, r in results.items() if not r["matches_expectation"]
            ),
        }
        # Confirm the declared build outputs against the gate that produces them.
        for spec in DECLARED_BUILD_OUTPUTS:
            record = manifest["digests"][spec["id"]]
            gate_result = results.get(spec["produced_by_gate"])
            observed = None
            if gate_result:
                prefix = spec["produced_by_output_key"] + "="
                for line in gate_result["key_lines"]:
                    if line.startswith(prefix):
                        observed = line[len(prefix):]
                        break
            record["observed_sha256"] = observed
            record["matches_handoff"] = observed == spec["handoff"]
        if manifest["gate_summary"]["contradicting_expectation"]:
            exit_code = EXIT_DRIFT

    manifest["unavailable_or_failing_gates"] = summarize_unavailable(gates, results)
    manifest["handoff_digest_disagreements"] = sorted(
        entry_id
        for entry_id, rec in manifest["digests"].items()
        if "handoff_declared_sha256" in rec and rec.get("matches_handoff") is False
    )
    return manifest, exit_code


# --------------------------------------------------------------------------
# check
# --------------------------------------------------------------------------

IGNORED_TOP_LEVEL = {"run"}


def check_manifest(
    repo: Path, manifest_path: Path, *, with_gates: bool, gate_timeout: int, script_path: Path
) -> int:
    if not manifest_path.is_file():
        die(f"{manifest_path} does not exist; nothing to check")
    recorded = json.loads(manifest_path.read_text(encoding="utf-8"))
    if recorded.get("schema") != SCHEMA:
        die(f"unsupported schema {recorded.get('schema')!r}; expected {SCHEMA!r}")

    drift: list[str] = []
    notes: list[str] = []

    current_generator = sha256_file(script_path)
    if recorded.get("generator", {}).get("sha256") != current_generator:
        drift.append(
            "generator: scripts/baseline_manifest.py digest changed "
            f"({recorded.get('generator', {}).get('sha256')} -> {current_generator}); "
            "the manifest is no longer reproducible by the current generator"
        )

    try:
        out_rel = str(manifest_path.resolve().relative_to(repo))
    except ValueError:
        out_rel = str(manifest_path)
    worktree = collect_worktree_state(repo, [out_rel])
    if bool(recorded.get("dirty")) != worktree["dirty"]:
        drift.append(
            f"dirty: recorded {bool(recorded.get('dirty'))}, observed {worktree['dirty']}"
        )
    elif worktree["dirty"]:
        if recorded.get("dirty_porcelain", []) != worktree["porcelain"]:
            notes.append(
                "dirty_porcelain: the set of uncommitted paths changed since the "
                "snapshot was taken (informational; a dirty snapshot is never a baseline)"
            )

    content_identity = collect_content_identity(repo, [out_rel])
    recorded_identity = recorded.get("baseline", {}).get("content_identity", {})
    for field in ("schema", "algorithm", "sha256", "entry_count", "excluded_paths"):
        if recorded_identity.get(field) != content_identity[field]:
            drift.append(
                f"baseline.content_identity.{field}: recorded "
                f"{recorded_identity.get(field)}, observed {content_identity[field]}"
            )

    digests = collect_digests(repo)["entries"]
    recorded_digests = recorded.get("digests", {})
    for entry_id in sorted(set(recorded_digests) | set(digests)):
        if entry_id not in recorded_digests:
            drift.append(f"digests.{entry_id}: present now, absent from the manifest")
            continue
        if entry_id not in digests:
            drift.append(f"digests.{entry_id}: in the manifest, no longer derivable")
            continue
        rec = recorded_digests[entry_id]
        cur = digests[entry_id]
        if cur["kind"] == "declared-build-output":
            continue
        if rec.get("sha256") != cur.get("sha256"):
            drift.append(
                f"digests.{entry_id} ({cur.get('path')}): recorded "
                f"{rec.get('sha256')}, observed {cur.get('sha256')}"
            )

    toolchain = collect_toolchain(repo)
    for key in ("versions_env", "pinned_proof_tools_md"):
        rec = recorded.get("toolchain", {}).get("records", {}).get(key, {}).get("sha256")
        cur = toolchain["records"][key]["sha256"]
        if rec != cur:
            drift.append(f"toolchain.records.{key}: recorded {rec}, observed {cur}")
    if recorded.get("toolchain", {}).get("pin_agreement", {}).get("all_agree") != toolchain[
        "pin_agreement"
    ]["all_agree"]:
        drift.append("toolchain.pin_agreement.all_agree changed")

    gates = build_gates()
    recorded_gates = recorded.get("gates", [])
    current_gate_records = [gate_manifest_record(gate) for gate in gates]
    recorded_gate_ids = [g["id"] for g in recorded_gates]
    current_gate_ids = [g["id"] for g in current_gate_records]
    if recorded_gate_ids != current_gate_ids:
        drift.append(
            "gates: the declared gate set changed "
            f"(recorded {len(recorded_gate_ids)}, current {len(current_gate_ids)})"
        )
    elif recorded_gates != current_gate_records:
        for recorded_gate, current_gate in zip(recorded_gates, current_gate_records):
            if recorded_gate != current_gate:
                drift.append(
                    f"gates.{current_gate['id']}: declaration changed "
                    "(command, expectation, output selection, or classification)"
                )

    if with_gates:
        recorded_runs = recorded.get("gate_runs")
        if not recorded_runs:
            drift.append(
                "gate_runs: --run-gates was requested but the manifest records no "
                "gate run to compare against"
            )
        else:
            print("baseline_manifest: re-running gates for check", file=sys.stderr)
            results = run_gates(repo, gates, gate_timeout)
            for gate in gates:
                gid = gate["id"]
                rec = recorded_runs.get(gid)
                cur = results[gid]
                if rec is None:
                    drift.append(f"gate_runs.{gid}: absent from the manifest")
                    continue
                if rec.get("exit_code") != cur["exit_code"]:
                    drift.append(
                        f"gate_runs.{gid}.exit_code: recorded {rec.get('exit_code')}, "
                        f"observed {cur['exit_code']}"
                    )
                if rec.get("key_lines_sha256") != cur["key_lines_sha256"]:
                    drift.append(
                        f"gate_runs.{gid}.key_lines: recorded digest "
                        f"{rec.get('key_lines_sha256')}, observed {cur['key_lines_sha256']}"
                    )
                if not cur["matches_expectation"] and rec.get("matches_expectation"):
                    drift.append(
                        f"gate_runs.{gid}: previously matched its declaration, now does not"
                    )

    for note in notes:
        print(f"note: {note}")
    if drift:
        print(f"DRIFT: {len(drift)} mismatch(es) against {manifest_path}")
        for item in drift:
            print(f"  - {item}")
        return EXIT_DRIFT
    scope = "digests, toolchain records, gate declarations"
    if with_gates:
        scope += ", gate exit codes and key output lines"
    print(f"OK: {manifest_path} matches the working tree ({scope}).")
    if recorded.get("dirty"):
        print(
            "  reminder: this manifest is a dirty mid-flight snapshot, not a baseline."
        )
    return EXIT_OK


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="baseline_manifest.py",
        description="Generate and check the Dragon's Clutch baseline evidence manifest.",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    emit = sub.add_parser("emit", help="derive and write MANIFEST.baseline.json")
    emit.add_argument("--out", type=Path, default=None)
    dirty_group = emit.add_mutually_exclusive_group()
    dirty_group.add_argument(
        "--strict",
        action="store_true",
        help="refuse to emit on a dirty working tree (the default)",
    )
    dirty_group.add_argument(
        "--allow-dirty",
        action="store_true",
        help="emit a labelled mid-flight snapshot from a dirty tree",
    )
    emit.add_argument(
        "--run-gates",
        action="store_true",
        help="execute every declared gate and record exit codes and key output lines",
    )
    emit.add_argument("--gate-timeout", type=int, default=DEFAULT_GATE_TIMEOUT)

    check = sub.add_parser("check", help="re-derive and report drift against a manifest")
    check.add_argument("--manifest", type=Path, default=None)
    check.add_argument(
        "--run-gates",
        action="store_true",
        help="also re-run every gate and compare exit codes and key output lines",
    )
    check.add_argument("--gate-timeout", type=int, default=DEFAULT_GATE_TIMEOUT)

    args = parser.parse_args(argv)
    script_path = Path(__file__).resolve()
    repo = repo_root(script_path.parent)

    if args.command == "emit":
        out = args.out or (repo / DEFAULT_MANIFEST)
        try:
            out_rel = str(out.resolve().relative_to(repo))
        except ValueError:
            out_rel = str(out)
        manifest, code = build_manifest(
            repo,
            allow_dirty=args.allow_dirty,
            with_gates=args.run_gates,
            gate_timeout=args.gate_timeout,
            script_path=script_path,
            out_rel=out_rel,
        )
        out.write_text(
            json.dumps(manifest, indent=2, sort_keys=False, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        print(f"wrote {out}")
        print(f"  schema        {manifest['schema']}")
        print(
            "  source sha256 ",
            manifest["baseline"]["content_identity"]["sha256"],
        )
        print(
            "  source entries",
            manifest["baseline"]["content_identity"]["entry_count"],
        )
        print(
            "  provenance    ",
            manifest["baseline"]["provenance"]["emitted_from_commit"],
        )
        print(f"  dirty         {manifest['dirty']}")
        print(f"  digests       {len(manifest['digests'])}")
        print(f"  gates         {len(manifest['gates'])}")
        if "gate_summary" in manifest:
            summary = manifest["gate_summary"]
            print(
                f"  gate outcomes {summary['matching_expectation']}/{summary['total']} "
                "match their declaration"
            )
            for gid in summary["contradicting_expectation"]:
                print(f"    CONTRADICTS DECLARATION: {gid}")
        if manifest["handoff_digest_disagreements"]:
            print("  handoff digest disagreements:")
            for gid in manifest["handoff_digest_disagreements"]:
                print(f"    {gid}")
        return code

    manifest_path = args.manifest or (repo / DEFAULT_MANIFEST)
    return check_manifest(
        repo,
        manifest_path,
        with_gates=args.run_gates,
        gate_timeout=args.gate_timeout,
        script_path=script_path,
    )


if __name__ == "__main__":
    raise SystemExit(main())
