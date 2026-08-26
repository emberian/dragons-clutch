# Liability basis V2 kernel

Status: pure semantic and differential slice. It is not a capability release,
Market migration, SBF artifact, or deployment claim.

This crate is a handwritten safe `no_std`, `no_alloc`, runtime-width kernel for
the Lean-owned `LiabilityBasisV2` contract. For an integer payout vector `p`,
positive scale `Q`, supplies `T`, and Hoard collateral `H`:

```text
sum p_i = Q
L(T,p) = sum T_i * p_i
L(T + q*1, p) = L(T,p) + q*Q
H >= L(T,p)  =>  H + q*Q >= L(T + q*1,p)
```

The first concrete evaluator is a two-claim capped ramp. Its one named
apportionment boundary is `capped_ramp_complement_floor_boundary_v2`:

```text
primary    = floor(Q * elapsed / width)
complement = Q - primary
```

Tails clamp to `[0,Q]` and `[Q,0]`. No second rounding or remainder exists.
Categorical claims embed as the runtime-width `Q=1` one-hot profile.

The provisional physical request is exactly 64 bytes. It uses `u32` positive
scale and denominators plus `i64` signed numerators. Checked `i128` cross
products and `u128` interpolation products cover that complete profile. These
are physical representation bounds, not mathematical basis-width limits.

Complete-set merge, claim transfer, and single-claim terminal redemption are
executed by the same runtime-width planner as split. Liability and collateral
move by exactly the same amount in every admitted transition:

```text
split   L(T + q*1, p) = L(T,p) + q*Q      H' = H + q*Q
merge   L(T - q*1, p) + q*Q = L(T,p)      H' + q*Q = H
redeem  L(T - q*e_i, p) + q*p_i = L(T,p)  H' + q*p_i = H
trade   aggregate T unchanged, so L and H are both unchanged
```

`maximum_liability_v2` is the certified pre-resolution envelope `Q * peak(T)`.
Lean bounds exact liability by it for every basis and proves it attained for
both admitted evaluator families, so covering it is solvency at every admitted
terminal result without enumerating the result domain.

`src/generated.rs` contains only Lean-emitted ABI constants plus three corpora:
sixteen agreement cases, nineteen hostile refusal cases, and twenty-four
runtime-width transition cases covering split, merge, and terminal redemption
with every reachable refusal tag. The evaluator, hostile decoder, liability
arithmetic, and transition planners are handwritten Rust. Regeneration must go
to a temporary file and be compared before atomically replacing the accepted
generated file; `check-generated.sh` performs exactly that.

The crate is a canonical workspace member. Capability admission and Market
migration remain separately gated work.
