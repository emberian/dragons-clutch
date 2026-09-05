#!/usr/bin/env python3
"""Derive a cohort's seven semantic release ids from WHAT THEY IDENTIFY, and
state the accelerator's eighth.

  python3 tools/cohort/semantic-release-ids.py <job>/candidate/elf \
      <job>/candidate/elf/accelerator.so > <job>/semantic-release-ids.txt

Every emitted `prepare` stage reads that file through its own `semantic` helper,
so the ids reach the plan as a value the operator produced from the artifacts
rather than as a literal typed into a row. THIS IS A SECOND AUTHOR AND IT IS A
CHECKED ONE: `validate_prepare` re-derives all five artifact-derived ids from
the artifact each is supplied beside and refuses a mismatch by name, and the two
code-owned constants are compared against the constants themselves. What it is
NOT allowed to be is a hand-typed hex string, which is why this lives here and
not in a job directory.

A role's semantic release id is either a code-owned constant (Trading's
COMPILED_DIRECT_RELEASE_ID_V1, Resolution's RESOLUTION_CONTROLLER_RELEASE_ID_V7)
or the digest of a fixed domain, the role's canonical label and the role's
SHIPPED ELF DIGEST. No git revision appears anywhere in it, which is why an id
is a function of the bytes and not of the commit that produced them.

THE ACCELERATOR'S IS OPERATOR-STATED and cannot be otherwise:
`checked_semantic_release_preimage_v1` refuses any role outside the seven with
"role has no protocol-owned semantic release identity", so nothing in the tree
derives it. What is stated here is nonetheless REPRODUCIBLE rather than random:
it is the same artifact derivation under the label `general-accelerator`, which
no role uses, so it is a function of the shipped accelerator ELF and cannot
collide with any of the seven by construction. That is a stronger property than
the tool checks (nonzero, no collision) and it is still not a check -- the fix
is a SourceSemanticRoleV1::GeneralAccelerator label, after which the flag
becomes one.
"""
import hashlib, sys

ARTIFACT_DOMAIN_V2 = b"dclutch/checked-semantic-release/artifact/v2\n"
DIRECT_PREIMAGE_V1 = b"dclutch/release/direct-compiled-controller-v1"
RESOLUTION_PREIMAGE_V7 = (
    b"dclutch/release/source-resolution-controller-direct-activation-"
    b"receipt-permissionless-close-v7"
)
ROLES = [("registry", "registry"), ("core", "core"), ("claims", "claims"),
         ("custody", "custody"), ("rent-credit", "rent-credit")]


def artifact_id(label, elf_sha256):
    if len(elf_sha256) != 64 or any(c not in "0123456789abcdef" for c in elf_sha256):
        raise SystemExit(f"{label}: artifact digest is not 64 lowercase hex bytes")
    return hashlib.sha256(
        ARTIFACT_DOMAIN_V2 + label.encode() + b"\0" + elf_sha256.encode()).hexdigest()


def main():
    elfdir = sys.argv[1]
    accelerator_elf = sys.argv[2] if len(sys.argv) > 2 else None
    out = {}
    for flag, label in ROLES:
        role = "rent" if flag == "rent-credit" else flag
        digest = hashlib.sha256(open(f"{elfdir}/{role}.so", "rb").read()).hexdigest()
        out[flag] = (artifact_id(label, digest), digest)
    out["trading"] = (hashlib.sha256(DIRECT_PREIMAGE_V1).hexdigest(), "code-owned constant")
    out["resolution"] = (hashlib.sha256(RESOLUTION_PREIMAGE_V7).hexdigest(), "code-owned constant")
    if accelerator_elf:
        digest = hashlib.sha256(open(accelerator_elf, "rb").read()).hexdigest()
        out["general-accelerator"] = (artifact_id("general-accelerator", digest), digest)
    seven = {out[flag][0] for flag in out if flag != "general-accelerator"}
    if "general-accelerator" in out:
        identity = out["general-accelerator"][0]
        if identity in seven or int(identity, 16) == 0:
            raise SystemExit("the accelerator's stated id is zero or collides with a role's")
    for flag in sorted(out):
        print(f"{flag} {out[flag][0]}")
    for flag in sorted(out):
        print(f"# {flag:<20} from {out[flag][1]}", file=sys.stderr)


if __name__ == "__main__":
    main()
