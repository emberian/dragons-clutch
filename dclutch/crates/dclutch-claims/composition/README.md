# Representation Composition V3 Kernel

This standalone `no_std`, `no_alloc`, safe-Rust kernel authenticates and
flattens a bounded acyclic representation recipe into one canonical sparse
payoff over an exhaustive native liability basis.

The immutable descriptor is the only owner of the logical Market,
result-domain, selected release set, and native-basis identities. Graph and
translation records contain only their own stable identities and topology.
They cannot resolve a Product, select a terminal result, mutate Claims, mint or
burn Token-2022 assets, or pay collateral.

## Executable capacity profile

The pinned V3 profile admits at most 256 native outcomes, 32 nodes, 96 edges,
and 2,048 sparse terms. Arithmetic uses checked `u128`; overflow is a refusal.
These limits are encoded in the descriptor through a named capacity-profile
identity and are implementation capacity, not protocol ontology.

## Fixed records

- Descriptor: exactly 368 bytes.
- Graph: 112-byte header, then `node_count * 80`, `edge_count * 48`, and
  `term_count * 16` bytes.
- Translation: 128-byte header, then `term_count * 16` bytes.

Nodes are strictly ordered by `(rank, id)`. Native leaves are unique exhaustive
basis coordinates. Composed nodes have strictly ordered unique child IDs,
positive coefficients, a positive divisor, and a GCD-normalized recipe. Each
node carries a strictly ordered, positive, GCD-normalized sparse native payoff.
The decoder independently recomputes that payoff from its children using
checked integer LCM arithmetic and requires exact equality.

The only root is the final node. It has no incoming edge and every other node
has at least one incoming edge, which proves reachability in the backward-only
DAG. The translation term bytes must be byte-identical to the root term bytes.

## Adapter boundary

The caller supplies authenticated finalized-record evidence and SHA-256
digests. The small physical adapter remains responsible for Registry owner/PDA,
raw/staging finality, selected release, and exact content-digest checks. Claims
and Token-2022 remain the only physical balance and supply writers. Product/Core
remain the only result authority.

