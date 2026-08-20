# Why the ELF identity is same-path-reproducible, not path-independent

Status: **ROOT-CAUSED FINDING / PROTOCOL AMENDMENT.** Measured 2026-08-20
during the cycle-D identity check; supersedes the path-sensitivity notes in
the cycle-B and cycle-C audits with the actual mechanism.

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
