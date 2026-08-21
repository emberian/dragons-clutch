# Why the ELF identity is same-path-reproducible, not path-independent

Status: **ROOT-CAUSED FINDING / PROTOCOL AMENDMENT.** Measured 2026-08-20
during the cycle-D identity check; supersedes the path-sensitivity notes in
the cycle-B and cycle-C audits with the actual mechanism. The 2026-08-21
addendum below dispositions the NON-PRODUCTION mock profile against this
mechanism; it measures nothing and moves no seal.

## The mechanism, proven

Cargo derives each crate's `-C metadata` from its package identity, and for
**path dependencies the package identity includes the absolute workspace
path**. Every checkout location therefore produces a different set of
mangled-symbol hash suffixes for the entire first-party graph (verified:
`llvm-nm` on two same-commit builds shows identical addresses with different
`17h…` suffixes on every first-party symbol).

Layout is almost entirely determined by stable keys, so this is invisible —
until two symbols tie on every stable key and the linker orders them **by
hash**. Then their order is a deterministic function of the checkout path,
and the binary differs by a few bytes per tied pair (measured: a two-function
swap, 5 bytes at 4 sites — two jump-target bytes exchanged plus two relative
offsets). The first such tie entered with the T2-7 merge; a larger tied
group (~490 bytes) appeared transiently in the T2-6-era comparisons.

Consequences, stated exactly:
- A build is **byte-reproducible at the same absolute path** (verified: two
  fresh-target builds at each of five paths, byte-identical per path).
- Builds at different paths differ by the tied-pair signature only — same
  length, same code, swapped placement (semantics-neutral).
- The prior seals' "source-path-independent" observations were true of
  artifacts that happened to contain no hash-sorted ties, not a property of
  the build.

## Protocol amendment

The canonical artifact identity is defined by the canonical checkout path,
`/Users/ember/dev/dragons-clutch`, where every bank log's fixture binds. The
seal's clean-build guarantee moves from "detached worktree" to: verify the
canonical tree is exactly HEAD (`git status --porcelain` empty and
`git diff HEAD` empty over the closure), then double-build with fresh target
directories **in place**. Cross-path builds are recorded as the relocation
probe with disposition `PATH_TIED_SYMBOL_ORDER` and their observed digests
listed. Cross-path byte-reproduction would require either building at the
pinned path (containers do this trivially) or eliminating hash-sorted ties,
which no single source change can guarantee against forever.

## Addendum, 2026-08-21: the NON-PRODUCTION mock ELF

Status: **DISPOSITION OF A CARRIED OBSERVATION. NOT MEASURED HERE.**

The general-plane signed validator walk left a follow-up (`7be4d66`,
`GOAL.md` "NEW FOLLOW-UPS"): *the mock ELF is build-path-sensitive at this
HEAD (eBPF code clusters, pre-existing — register-worthy)*, at the
`e8ba31d5…` identity the walk confirmed on merged main. The swarm carries it
in item J as "mock path-sensitivity", and as carried it names three paths and
three distinct hashes. What is **committed** is the qualitative claim and the
code-cluster signature; the three digests are not in the tree, so that count
travels as a carried observation with no committed evidence behind it, and is
recorded here as such rather than repeated as a measurement.

**This addendum did not re-measure it.** The two-path build was gated on the
suite spinlock being free and the machine quiet. At both checks
`/tmp/claude-501/suite.lock` was held by another lane, with two SVM suites
running and a load average above ten. The swarm has priority, and a build run
under that contention produces a number nobody can interpret anyway. The
experiment was skipped, not deferred silently. What follows is disposition.

### The mechanism is almost certainly this document's

`programs/clutch-sbf/scripts/run_bringup.sh` builds both profiles through one
recipe against one manifest, differing **only** by
`--features non-production-mock-source`. Same workspace, same path
dependencies, same `-C metadata` derivation. So the account proven above —
package identity includes the absolute workspace path for path deps, so every
checkout produces a different set of mangled-symbol hash suffixes, and symbols
that tie on every stable key are then ordered by hash — applies to the mock
profile unchanged, and the reported "eBPF code clusters" is the same
`.text`-cluster signature the default profile shows.

This is inference from a shared recipe, not a measurement of the mock
artifact. The falsifier is cheap and named at the end.

### The disposition: nothing governed moves

Three facts, each checkable in the tree today.

1. **The seal does not bind the mock ELF, and says so.**
   `research/liveness-policy-profile/policy.py` and `evidence.json` contain no
   reference to the mock profile at all. The sealed `artifact_reproducibility`
   block binds exactly one artifact — the default ELF — with its
   `cross_path_builds` observed-digest list, `cross_path_disposition`,
   relocation probe, and relocation controls. The mock ELF appears in no seal
   root, no historical root set, and no portable attestation, and the sealed
   audit states the boundary outright: "no mock-feature ELF evidence is mixed
   here" (`artifacts/4fded7a67a2d8994/audit/RUNTIME_ARTIFACT_AUDIT.md`).

2. **The manifest pins no mock digest, deliberately.**
   `DECLARED_BUILD_OUTPUTS`'s `clutch_sbf.non_production_mock_program_elf`
   carries `handoff: None`, with the reason written into the record: "fresh
   same-machine identity observed only when its gate runs", and "local test
   evidence only … not a production-provider, deployment, or release
   identity". Its `sbf.runtime_bringup` key pattern is a *shape* regex,
   `^non_production_mock_sbf_elf_sha256=[0-9a-f]{64}$`, which matches any
   digest by construction. (The default ELF's is the same shape; only
   `toolchain.e0_sbf_rlib` carries a pinned handoff.)

3. **What the gate does assert is same-path reproducibility — exactly the
   property the protocol amendment above defines.** `run_bringup.sh` builds
   each profile twice into fresh target directories at the canonical path and
   fails if a profile's two builds differ (`default_reproducibility=PASS`,
   `mock_reproducibility=PASS`), plus `profile_separation=PASS` for the
   separate obligation that the two profiles must *not* collide. Nothing
   cross-path is asserted for either profile.

The consequence, plainly: the mock ELF being build-path-sensitive contradicts
no published claim, invalidates no seal, and changes no disposition. It is the
expected behavior of this build under the mechanism proven above, noticed on
the one artifact where it costs nothing.

### The default profile's tie behavior is the governed one

Any statement about this project's build reproducibility should be read
against the default ELF, because that is the artifact the seal binds and the
one a deployment would carry. At the cycle-F root its cross-path divergence is
recorded as an observed-digest list with disposition
`PATH_TIED_SYMBOL_ORDER` and divergence
`481_TEXT_BYTES_AT_195_SITES_AND_6_REL_DYN_BYTES_AT_3_SITES_NO_OTHER_SECTION`,
and `policy.py::check_artifact_binding` refuses both the retired scalar field
and any list entry equal to the canonical digest. The mock profile's behavior
is a test artifact's, ungoverned by construction, and must never be quoted as
evidence about the deployable one in either direction.

Nor is the tie population stable — across waves *or across profiles* — which
is the trap here, and the walk lane's observation is the sharpest illustration
of it available. The walk confirmed `e8ba31d5…` on merged main, and that is
the cycle-D seal (root `e8ba31d582be3939`) whose headline was that the final
artifact carried **no surviving hash-sorted tie at all**: stripped cross-path
builds were byte-identical again and Cargo-home relocation read `INDEPENDENT`.
So at one HEAD, by the carried observation, the mock profile diverged across
paths while the default profile did not.

That is not a contradiction; it is the mechanism behaving as described. A tie
requires two symbols equal on every stable key, so the tie population is a
property of the *symbol set*, and `--features non-production-mock-source`
changes the symbol set. Two profiles of one commit can and do sit on opposite
sides of the tie question. Which means neither profile's cross-path result
predicts the other's at any HEAD, in either direction, and "byte-identical
across paths" is never a property of this build — only ever an observation
about one artifact at one wave.

### What would settle it, when the tree is quiet

Bounded, and worth exactly one slot behind the lock:

1. `programs/clutch-sbf/scripts/run_bringup.sh` at the canonical path; keep
   `non_production_mock_sbf_elf_sha256`.
2. The same commit in a detached worktree at a second path — same recipe,
   fresh target directory, mock feature on.
3. `cmp -l` the two mock ELFs and classify the differing offsets by section.

Predicted, and therefore falsifiable: the differing bytes fall entirely in
`.text` (with `.rel.dyn` possibly following), every other section stays
byte-identical, sizes match, and the disassembly line count is unchanged —
the signature this document records for the default profile. Anything else —
`.rodata` growth, a size change, a changed instruction count — refutes the
shared-mechanism reading and earns a real investigation.

Record the result as an observation. Pinning a mock digest would manufacture
exactly the release-shaped claim `DECLARED_BUILD_OUTPUTS` refuses to make.
