# dClutch successor Resolution controller

This is the first physical specialization of the Lean-owned Source Resolution
relation. It handles one deliberately narrow hot path: a primary, terminal,
already-posted Pyth `PriceUpdateV2` whose Receiver verification level is
`Full`.

The program does not post or reclaim provider accounts and performs no
provider CPI. The Pyth update is read-only. Before producing any mutation it
authenticates:

- the Core-owned open Market and Market-bound Source state;
- the finalized execution-authority manifest selected by the Market;
- the Registry-owned activated release-set PDA and its Resolution role;
- its own current Loader V3 Program, ProgramData, complete ELF digest,
  deployment slot, and upgrade-authority policy against that role;
- finalized Source-material, Product result-domain, and Pyth-release records;
- every embedded Source component content identity used by the hot path;
- Pyth Receiver/router ProgramData links and deployment slots;
- the Receiver configuration digest/router binding and fully verified update;
- the canonical Clock and Rent sysvars; and
- exact generation, Product-domain, provider-release, PDA, owner, privilege,
  account-order, and non-alias coordinates.

It then delegates normalization, timing, confidence, exact Product mapping,
and Source lifecycle mutation to `dclutch-source-contract`. Only after a full
candidate state and Lean-layout 312-byte certificate have been built and both
writable accounts have been borrowed does it copy either output. A refusal
therefore leaves both accounts unchanged even in direct host invocation; SVM
transaction rollback remains an additional runtime boundary.

This slice does **not** yet execute the Product-resolution effect against the
Core/claims owner, spend capability funding, advance recovery, commit failure,
create accounts, or retire them. The certificate is the compact handoff to
that shared executor work. Registry/Core account mutation, Loader behavior,
Pyth program correctness, Clock correctness, SHA-256 syscall correctness, and
SVM rollback are not Lean proofs.
