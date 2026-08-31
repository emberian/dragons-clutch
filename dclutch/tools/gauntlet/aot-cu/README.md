# Direct relation interpreted-vs-AOT CU harness

One twin crate is built three times from the same source. The request decode,
bank decode, digesting, acknowledgement construction and encode are shared
verbatim; only `evaluate_relation` differs:

  --features aot          the hand-written straight-line translation
  --features interpreted  ProgramV2::decode + the generic TransitionVM
  --features null         the frame with the relation removed

so a CU difference between the ELFs is the evaluator and nothing else. The
shipped `dclutch-direct-aot-sbf` ELF is loaded alongside them and must agree
with the AOT twin on every acknowledgement byte, which is what makes the twin
frame trustworthy rather than merely plausible.

## Running

    cargo build-sbf --manifest-path twin/Cargo.toml --features aot
    cp target/deploy/dclutch_direct_relation_twin.so "$ELFS/direct_relation_twin_aot.so"
    cargo build-sbf --manifest-path twin/Cargo.toml --features interpreted
    cp target/deploy/dclutch_direct_relation_twin.so "$ELFS/direct_relation_twin_interpreted.so"
    cargo build-sbf --manifest-path twin/Cargo.toml --features null
    cp target/deploy/dclutch_direct_relation_twin.so "$ELFS/direct_relation_twin_null.so"
    cp ../../../target/deploy/dclutch_direct_aot_sbf.so "$ELFS/"

    SBF_OUT_DIR="$ELFS" cargo test --manifest-path harness/Cargo.toml \
        --test measure -- --ignored --nocapture

`$ELFS` is any staging directory; the three twin builds share one cdylib name,
so each must be copied out before the next build overwrites it.

## Seeds

Thirty-two, deterministic. Seed 0 is the formal example. Seeds 1..24 are
admissible *by construction* -- the relation is a long conjunction including
`fill == maximum` under FOK, `nonce == next_nonce`, three equal fee rates and an
exact `fill * price / scale`, so independently sampled banks refuse almost
immediately and would price the early-exit path while claiming to price the
route. Seeds 24..32 each violate one conjunct, chosen to land at different
depths, because refusal depth is where an interpreter loses most.

## The current relation (`twin-v3`)

`twin/` prices the V2 descriptor, which is what the shipped accelerator ELF
evaluates. The live route runs the InlineOrdinary TransitionVMV3 program
instead -- 70 instructions, 1,712 bytes, folded over the Product tail -- so
`twin-v3/` prices that one the same three ways.

Its `null` build performs the same bank construction and transition encode as
the other two, so those costs cancel in the differences rather than being
attributed to either evaluator. The encode is a harness convenience: it stands
in for however the route obtains the program bytes, and because it is identical
in all three builds it cannot bias the comparison.

**The AOT column cannot be reproduced from a bare checkout.**
`dclutch-direct-aot-v3-contract` does not compile for `target_os = "solana"`:
its `registered` module uses the V4 register schema unconditionally, and
`crates/dclutch-direct-codec/src/registered_fill_artifacts_v4.rs:39` publishes
that schema only under `#[cfg(not(target_os = "solana"))]`. Building the AOT
ELF requires gating the module for Solana, which was done locally and
deliberately not committed:

    // crates/dclutch-direct-aot-v3-contract/src/lib.rs
    #[cfg(not(target_os = "solana"))]
    mod registered;
    #[cfg(not(target_os = "solana"))]
    pub use registered::execute_registered_ordinary_fill_atomic;

That two-line gate is the whole of what stands between the current AOT
translation and an SBF artifact, and it is a protocol-crate change, so it
belongs to whoever owns that crate rather than to a measurement lane.

    cargo build-sbf --manifest-path twin-v3/Cargo.toml --features aot
    cp target/deploy/dclutch_direct_relation_twin_v3.so "$ELFS/direct_relation_twin_v3_aot.so"
    # ... likewise for interpreted and null
    SBF_OUT_DIR="$ELFS" cargo test --manifest-path harness/Cargo.toml \
        --test measure_v3 -- --ignored --nocapture

### The decode-only build, and why it exists

`twin-v3` has a fourth build, `--features decode-only`, which runs
`ProgramV3::decode` and stops. It is there because the first version of this
measurement was wrong in AOT's favour by a factor of 2.3.

The live route calls `TransitionProgramV3::from_sealed`
(`programs/dclutch-trading-sbf/src/hot_v3.rs:2250`), which skips
`validate_body` -- the per-instruction sweep, and the expensive half of
`decode` -- because the write-once seal instruction already ran it. A twin that
calls `decode` therefore charges the interpreter 13,481 CU per invocation that
the route pays once, at seal time. Subtracting the decode-only build leaves the
fold alone, which is the only part an AOT translation displaces.

Anything comparing an interpreter against a compiled form has to answer the same
question first: which half of the decode does the caller actually pay?
