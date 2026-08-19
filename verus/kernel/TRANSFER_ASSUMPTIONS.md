# Transfer-arithmetic refinement assumptions

The narrow `prepare_internal_transfer` result depends on:

1. the soundness of pinned Verus, its vstd specifications, its Rust 1.97.1
   frontend, and bundled Z3 4.16.0;
2. the proof runner preserving the production executable body while converting
   documentation comments, naming the return value, and inserting the displayed
   contract;
3. human review of the digest-pinned `MarketState::transfer_internal` call,
   error-map, and delayed-write seam;
4. agreement, for this conservative arithmetic subset, between the verifier's
   Rust frontend and the host/SBF compilers that compile the production source;
5. the specification actually expressing the intended two-owner conservation
   property; and
6. ordinary cryptographic collision resistance for the SHA-256 drift pins.

The result assumes no wallet, account, token program, Solana runtime, SBF
compiler, deployment, or network fact.  It does not authenticate semantic
owners or prove rollback outside the digest-pinned delayed-write seam.
