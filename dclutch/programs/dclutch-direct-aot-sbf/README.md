# Direct stateless AOT SBF

This isolated program accepts no accounts and performs no CPI or state write.
It hostile-decodes one `AcceleratorRequestV1`, evaluates the exact Lean-emitted
Direct V2 register relation through `dclutch-direct-aot-contract`, and returns
one `AcceleratorAckV1`. Semantic refusal returns a successful CPI with a refusal
ack; malformed physical input returns a program error.

This artifact is comparison-only. Canonical Trading must authenticate the
request coordinates and artifact, run the descriptor interpreter over the same
input, require exact acceptance/refusal and accepted-bank equality, project one
common effect, and commit once. AOT-only execution remains unavailable until
Registry owns descriptor/certificate/artifact admission or a checked proof
route establishes the same immutable relation.

The ordinary Direct profile is 584 request bytes and at most 616 return bytes.
A direct one-instruction, one-fee-payer v0 transaction is 756 bytes under the
pinned local SDK, below the 1,232-byte packet limit. The accelerator holds no
account, so its persistent rent is exactly zero. That direct packet measurement
does not claim that the eventual full Trading comparison frame already fits;
the integrated frame must be measured separately after its account suffix is
frozen.
