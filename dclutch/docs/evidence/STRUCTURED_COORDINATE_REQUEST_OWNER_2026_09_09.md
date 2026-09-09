# Structured nested request convergence — 2026-09-09

The exact nested protocol-Position request now has one production author:
`dclutch_claims::rational_lifecycle::LifecycleRequestV2::protocol_position_request`.
Trading's secondary signer, native Claims lifecycle execution and the Structured
host campaign all consume it. This converges the duplicated projection added
at the boundary recorded in `STRUCTURED_COORDINATE_NATIVE_BOUNDARY_2026_09_09.md`.

The kernel chooses Admit/Vacant or Close/Existing from the lifecycle action and
reads the request's sole coordinate. It accepts no action, owner or row override.
Adapters still hash the exact specialized V2 lifecycle wire, derive/check the
physical PDAs, enforce privileges and authenticate state. Native Claims retains
an explicit equality between the projected owner and its authenticated account.
The located projection error preserves the nested protocol-Position cause;
native adapters retain their existing refusal codes and log that cause.

Validation used the live repository `/Users/ember/dev/dclutch`, base HEAD
`e182f89e471228465079d39ca4c5c6d0d399f74a` plus this lane's owned changes, Rust
1.97.1 and this checkout's workspace target. Root and HEAD were printed with
the final commands:

- `cargo check --locked -p dclutch-trading-sbf -p dclutch-claims-sbf -p dclutch-journey-campaign`: passed.
- `cargo test --locked -p dclutch-claims --lib nested_position_projection_ -- --test-threads=1`: two passed. Independent expected bytes cover both actions with distinct rent values. Receipt-wide actions, zero digest and insufficient Position rent refuse their exact named causes.
- A temporary mutation replacing Position rent principal with observed Position lamports made both kernel controls red: the expected bytes differed, and the insufficient-rent row incorrectly became accepted instead of returning `CoordinatePositionRequestErrorV2::Position(ProtocolPositionErrorV2::InvalidRent)`. The mutation was removed before the final passing run.
- `cargo test --locked -p dclutch-trading-sbf --lib rational_coordinate_ -- --test-threads=1`: two passed, preserving the independently constructed expected PDA and six signer-boundary refusals.

No SBF build or runtime lifecycle execution is claimed here. This commit leaves
the frame ratchet red. The next exact-source cohort must capture its frames and
execute Structured activation and retirement; the retained pre-V2 validator was
not altered.
