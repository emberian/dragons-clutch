# dclutch-product-runtime-v2

This standalone successor removes the V1 const-generic and 16-outcome artifact
profile from Product result domains and rational portfolios. Both records use
exact hostile-decodable runtime tails with `u32` counts. Their only physical
width bound is the encoded record length and caller-provided account/buffer,
not a protocol semantic ceiling.

The Product-owned domain persists the selected liability-basis and
representation semantic identities. A portfolio references those identities
plus the authenticated domain content ID; `join_product_v2` refuses every
substitution. Hash computation and account authentication remain adapter trust
boundaries.

Lean owns the partition, exact-rational floor, and ABI constants. The Rust
kernel is safe `no_std`, `no_alloc`, borrowed-slice code. It applies one named
rounding boundary: final floor from exact rational representation to integer
claim atoms.
