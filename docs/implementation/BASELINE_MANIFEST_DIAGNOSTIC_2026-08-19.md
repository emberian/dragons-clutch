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

`b5700a9` re-sealed the current runtime/profile rather than weakening that
refusal. The current local artifact is:

- default ELF SHA-256:
  `bd20711b01828a745ce89de3aacb4b908cbcde32307b61be2c7d612bb8516b60`;
- runtime-artifact audit report SHA-256:
  `626a299dd879cff5f8c775b82b488c2d6b300a386b6d5f847913b5e14797e038`; and
- 52-file upstream/audit ledger SHA-256:
  `dbf55f8e28c1674fc0f76b434049fbc8ef1e906c46db6ac0457410eaebc35f35`.

The re-seal binds the current source closure and reruns the local same-ELF
profile evidence. It does not convert any local build, validator, bank, or
profile result into deployment, release, security-review, provider, global
liveness, or terminal-closure evidence.

## Historical boundary

The preceding `a5725a3d…` ELF, runtime source `7e8f6b1`, and `b5da74f`
evidence seal remain historical evidence only. Their validity for their
recorded source/artifact is not retracted; they are not relabelled as the
current profile. The current `bd20711b…` profile supersedes them only for the
current local source closure.

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

The remaining evidence step is a fresh Persvati attestation against that
checked endpoint. This remains local evidence bookkeeping, not authorization
to publish, sign, deploy, or release.
