# License and provenance policy

Status: applies now to research, documentation, fixtures, generated artifacts,
and future source.

## 1. First-party license

First-party Dragon's Clutch source and documentation are intended to be licensed
under `AGPL-3.0-or-later`, as recorded in the repository [LICENSE](../LICENSE).
Every public release must include the license, corresponding source, build and
installation material required by the license, and appropriate notices.

This file is a project process, not legal advice and not a substitute for a
release-time license review.

## 2. Greenfield boundary

Dragon's Clutch is a separate implementation. Do not copy, import, translate, or
depend on source from JOSHI, joshibot, leanuweave, minidregg, breadstuffs, Oracle
Pit, historical DREGG prototypes, or another local repository without an explicit
current user decision and a recorded provenance/license review.

Prior ideas may inform a human-authored clean-room specification. The record must
distinguish a general concept or public interface from copied expressive source.
Commit history or a local path is not license permission.

## 3. Dependency admission

Before adding a dependency, record:

```text
name and purpose
upstream repository/package
exact version and commit/content digest
license and notice obligations
source availability
maintainer/release authenticity
features enabled
transitive dependency lock digest
runtime/proof/build/dev classification
security and reproducibility notes
reviewer and date
```

Proof dependencies need the same review as runtime dependencies. A trusted
specification, axiom, code generator, compiler plugin, or binary tool expands the
evidence boundary even when it is not linked into the SBF ELF.

## 4. Fixtures and research inputs

Every nontrivial fixture or dataset records:

- source and acquisition date;
- applicable license/terms and redistribution permission;
- exact digest of the acquired input;
- derivation command/tool version and random seed;
- normalization, filtering, and unit conversions;
- synthetic versus observed status;
- whether it may enter the public repository.

Synthetic fixtures must not silently contain copied wallet, provider, or trading
data. No fixture may contain secrets, private keys, seed phrases, RPC credentials,
browser sessions, personal data without an explicit lawful basis, or unexplained
real account activity.

## 5. Generated artifacts

Generated wire types, proof output, vectors, SBF ELFs, static bundles, diagrams,
and benchmark reports must identify:

- the generator and exact version;
- all source/config/input digests;
- a deterministic reproduction command where possible;
- whether the artifact is reviewed source, build output, or evidence only;
- license/notice handling for embedded third-party material.

Generated files never become a second semantic owner. The checked schema or model
from which they derive remains authoritative.

## 6. Contribution checklist

Every future contribution should answer:

- [ ] Is the work original or is every incorporated source identified?
- [ ] Is it compatible with AGPL-3.0-or-later distribution?
- [ ] Are third-party notices and source-offer duties captured?
- [ ] Are fixtures/data redistributable and reproducible?
- [ ] Are generated artifacts clearly marked and rebuildable?
- [ ] Does it avoid the prohibited local-repository dependency boundary?
- [ ] Does it add any trusted proof, runtime, build, source, or hosted service?
- [ ] Are the SBOM and assumption manifests updated?

Unknown provenance blocks public release. Reimplementing an unknown-origin file
with superficial edits does not cure the problem.

## 7. Release ledger

An offline or public release candidate includes:

```text
LICENSE
NOTICE or third-party notices, when required
dependency and toolchain locks
SBOM
source-offer/reproduction instructions
fixture provenance manifests
generated-artifact manifest
proof assumption/trust inventory
source, vector, ELF, and static-bundle checksums
known provenance exceptions (normally empty)
```

No deployment, client mirror, or binary may be called official merely because it
was built from a repository with the correct license. Official identity requires
a checked release manifest and, for any separately authorized deployment, the
exact program-data manifest described elsewhere in this repository.
