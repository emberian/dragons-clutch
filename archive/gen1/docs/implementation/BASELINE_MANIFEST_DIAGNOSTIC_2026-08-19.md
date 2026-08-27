# Schema-v2 baseline diagnostic — 2026-08-19

Status: **DIAGNOSTIC / NOT A BASELINE / RELEASE STOP**.

## Current repair state

The first clean 94-gate run at `ec77d0b` matched 86 declarations and exposed
eight stale lock, tool, ABI, walk, and bringup assumptions. Those repair lanes
are now landed. Commit `83e124d` fixed the final runtime-bringup contradiction
by separating the inert default/`0x79` campaign from the explicitly
non-production mock-source success campaign.

The subsequent quiet-tree full run at `83e124d` reached **93/94**. Its sole
strict refusal was the liveness current-profile source-drift check, correctly
refusing to bind the old sealed profile to the changed runtime source. It was
not a release result and did not replace the checked schema-v1 manifest.

`b5700a9` re-sealed the runtime/profile rather than weakening that refusal.
The artifact it sealed, then current, was:

- default ELF SHA-256:
  `bd20711b01828a745ce89de3aacb4b908cbcde32307b61be2c7d612bb8516b60`;
- runtime-artifact audit report SHA-256:
  `626a299dd879cff5f8c775b82b488c2d6b300a386b6d5f847913b5e14797e038`; and
- 52-file upstream/audit ledger SHA-256:
  `dbf55f8e28c1674fc0f76b434049fbc8ef1e906c46db6ac0457410eaebc35f35`.

The later `7931e23` re-seal, for the Direct V3 runtime at source `2d530d2`,
supersedes that identity. The current local artifact is:

- default ELF SHA-256:
  `af6bb79cc3766bd0d889b46dc1becfebe140c7df2746971943e9edf4efc2014b`
  (`1,490,544` bytes, artifact root
  `research/liveness-policy-profile/artifacts/af6bb79cc3766bd0/`);
- runtime-artifact audit report SHA-256:
  `39a8b19cae23a2a02f7ba870b18b5a4b9a07af6876c05443d6dd28e8bb89ccfb`; and
- 50-file upstream/audit ledger SHA-256:
  `e433c17d4be57463e78eb47554cc6e84d22aab5c1a27a53e297f83a7a21304e0`.

Each re-seal binds its current source closure and reruns the local same-ELF
profile evidence. Neither converts any local build, validator, bank, or
profile result into deployment, release, security-review, provider, global
liveness, or terminal-closure evidence.

## Historical boundary

The `a5725a3d…` ELF, runtime source `7e8f6b1`, and `b5da74f` evidence seal
remain historical evidence only, as do the `bd20711b…` ELF, runtime source
`83e124d`, and `b5700a9` evidence seal that superseded them. Their validity
for their recorded source/artifact is not retracted; neither is relabelled as
the current profile, and each keeps its complete artifact directory. The
current `af6bb79c…` profile supersedes both only for the current local source
closure.

All named STOPs remain in force: no production provider or released source,
no global liveness/inclusion conclusion, Direct V2 remains a measured compute
STOP, no terminal closure, no independent reproducible-build closure, no
deployment, no release, no signature chain, and no security or legal closure.

## Final convergence result

The quiet-tree schema-v2 emission now records **94/94**, and the manifest-only
commit passes:

```sh
scripts/baseline_manifest.py check --run-gates
```

The fresh Persvati attestation against that checked endpoint has since
completed: exact `6743b9d` was independently attested from a fresh archive and
minimal hashed Git bundle with 40/40 portable gates PASS, 0 STOP, 528 files
checked twice with zero mismatches, and the then-sealed `bd20711b…` ELF
byte-verified on both hosts (recorded in `CURRENT_TRUTH.md` §2). This remains
local evidence bookkeeping, not authorization to publish, sign, deploy, or
release.

## Re-emission claim (14:35 local)

The post-seal wave (SBOM tool + TSV, hygiene fixes, truth updates,
terminal-identity-v1 crate; no gate-inventory or runtime-closure change)
drifted the recorded content identity 546->554 entries. COMPLETED: the clean-tree emission at
`c4688da` matched 98/98 declared gates (content identity 5d1feafc..., 554
entries), the manifest-only commit is `5b68601`, and the post-commit
`check --run-gates` passed with every gate exit code and key output line
matching. The tree is open for ordinary commits again.
